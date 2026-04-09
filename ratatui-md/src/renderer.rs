//! Core markdown-to-ratatui renderer.
//!
//! Converts parsed markdown into ratatui `Text` structures.

use crate::highlight::SyntaxHighlighter;
use crate::position_map::{CharMapping, FormatMark, PositionMap};
use crate::theme::Theme;
use md4c::{
    parse, Alignment, Block, BlockType, CodeBlockDetail, HeadingDetail, ImageDetail, LinkDetail,
    ListItemDetail, OrderedListDetail, ParserFlags, ParserHandler, Span, SpanType, TableCellDetail,
    TableDetail, TaskState, TextType, UnorderedListDetail, WikiLinkDetail,
};
use ratatui::style::Style;
use ratatui::text::{Line, Span as RSpan, Text};
use unicode_width::UnicodeWidthStr;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

// Thread-local cache for syntax-highlighted code blocks.
// Key is hash of (content, language), value is the highlighted lines.
// Capped at 64 entries to prevent unbounded growth in long-running apps.
const HIGHLIGHT_CACHE_CAP: usize = 64;
thread_local! {
    static HIGHLIGHT_CACHE: RefCell<HashMap<u64, Vec<Line<'static>>>> = RefCell::new(HashMap::new());
}

/// Compute a hash for a code block based on its content and language.
fn hash_code_block(content: &str, lang: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    lang.hash(&mut hasher);
    hasher.finish()
}

/// Render options for the markdown renderer.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Maximum width for wrapping (0 = no wrapping)
    pub width: usize,
    /// Parser flags for MD4C
    pub parser_flags: ParserFlags,
    /// Whether to include a blank line after headings
    pub heading_space: bool,
    /// Whether to include a blank line after paragraphs
    pub paragraph_space: bool,
    /// Whether to include a blank line after code blocks
    pub code_block_space: bool,
    /// Whether to include a blank line after lists
    pub list_space: bool,
    /// Whether to preserve soft breaks (single newlines) as line breaks
    /// instead of converting them to spaces (standard markdown behavior)
    pub hard_breaks: bool,
    /// Enable syntax highlighting for code blocks (requires `syntect` feature)
    pub syntax_highlighting: bool,
    /// Theme name for syntax highlighting (e.g., "base16-ocean.dark")
    pub syntax_theme: String,
    /// Track character positions for selection/extraction support.
    /// When enabled, builds a PositionMap with formatting context per character.
    pub track_positions: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: 0,
            parser_flags: ParserFlags::github(),
            heading_space: true,
            paragraph_space: true,
            code_block_space: true,
            list_space: true,
            hard_breaks: false,
            syntax_highlighting: false,
            syntax_theme: "base16-ocean.dark".to_string(),
            track_positions: false,
        }
    }
}

impl RenderOptions {
    /// Create options with CommonMark parsing.
    pub fn commonmark() -> Self {
        Self {
            parser_flags: ParserFlags::commonmark(),
            ..Default::default()
        }
    }

    /// Create options with GitHub Flavored Markdown parsing.
    pub fn github() -> Self {
        Self {
            parser_flags: ParserFlags::github(),
            ..Default::default()
        }
    }

    /// Set the maximum width for line wrapping.
    pub fn with_width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }

    /// Set parser flags.
    pub fn with_parser_flags(mut self, flags: ParserFlags) -> Self {
        self.parser_flags = flags;
        self
    }

    /// Treat soft breaks (single newlines) as hard breaks (line breaks).
    ///
    /// By default, markdown treats single newlines as spaces. Enable this
    /// to preserve single newlines as actual line breaks.
    pub fn with_hard_breaks(mut self, hard_breaks: bool) -> Self {
        self.hard_breaks = hard_breaks;
        self
    }

    /// Enable syntax highlighting for code blocks.
    ///
    /// Requires the `syntect` feature to be enabled.
    pub fn with_syntax_highlighting(mut self, enabled: bool) -> Self {
        self.syntax_highlighting = enabled;
        self
    }

    /// Set the theme for syntax highlighting.
    ///
    /// Available themes include: "base16-ocean.dark", "base16-eighties.dark",
    /// "base16-mocha.dark", "base16-ocean.light", "InspiredGitHub",
    /// "Solarized (dark)", "Solarized (light)"
    pub fn with_syntax_theme(mut self, theme: &str) -> Self {
        self.syntax_theme = theme.to_string();
        self
    }

    /// Enable LaTeX math span parsing and Unicode rendering.
    ///
    /// When enabled, `$...$` and `$$...$$` are parsed as LaTeX math spans
    /// and converted to Unicode approximations for terminal display.
    pub fn with_latex_math(mut self) -> Self {
        self.parser_flags = self.parser_flags.latex_math_spans();
        self
    }

    /// Enable position tracking for selection support.
    ///
    /// When enabled, the renderer builds a `PositionMap` that tracks
    /// formatting context for each rendered character. This is needed
    /// for text selection and extraction with markdown awareness.
    pub fn with_position_tracking(mut self, enabled: bool) -> Self {
        self.track_positions = enabled;
        self
    }
}

/// A rendered markdown document.
///
/// Contains the converted ratatui `Text` along with metadata about
/// links, headings, and other interactive elements.
#[derive(Debug, Clone)]
pub struct RenderedMarkdown {
    /// The rendered text content
    pub text: Text<'static>,
    /// Links found in the document: (line_index, start_col, end_col, url)
    pub links: Vec<LinkInfo>,
    /// Headings found in the document: (line_index, level, text)
    pub headings: Vec<HeadingInfo>,
    /// Total line count
    pub line_count: usize,
    /// Position map for character-level selection support.
    /// Only populated when `RenderOptions::track_positions` is true.
    pub position_map: Option<PositionMap>,
}

/// Information about a link in the rendered document.
#[derive(Debug, Clone)]
pub struct LinkInfo {
    /// Line index where the link appears
    pub line: usize,
    /// URL or target of the link
    pub url: String,
    /// Display text of the link
    pub text: String,
    /// Whether this is an autolink
    pub is_autolink: bool,
}

/// Information about a heading in the rendered document.
#[derive(Debug, Clone)]
pub struct HeadingInfo {
    /// Line index where the heading appears
    pub line: usize,
    /// Heading level (1-6)
    pub level: u8,
    /// Heading text content
    pub text: String,
}

/// Internal state for the renderer.
struct RendererState<'a> {
    theme: &'a Theme,
    options: &'a RenderOptions,

    // Output
    lines: Vec<Line<'static>>,
    current_spans: Vec<RSpan<'static>>,
    links: Vec<LinkInfo>,
    headings: Vec<HeadingInfo>,

    // Style stack for nested formatting
    style_stack: Vec<Style>,

    // Block context
    in_heading: Option<u8>,
    in_blockquote: bool,
    in_code_block: bool,
    code_block_lang: String,
    code_block_content: String, // Buffer for syntax highlighting
    in_list: bool,
    list_depth: usize,
    list_counters: Vec<u32>,
    list_is_ordered: Vec<bool>,
    current_task_state: Option<TaskState>,

    // Table state
    in_table: bool,
    table_columns: usize,
    table_alignments: Vec<Alignment>,
    table_rows: Vec<Vec<Vec<RSpan<'static>>>>,
    current_table_row: Vec<Vec<RSpan<'static>>>,
    current_table_cell: Vec<RSpan<'static>>,
    in_table_header: bool,

    // Link tracking
    current_link: Option<LinkDetail>,
    current_link_text: String,

    // Paragraph tracking
    pending_newline: bool,

    // Whether we need to add a list item prefix on the next text
    needs_list_prefix: bool,

    // LaTeX math buffering
    in_latex_math: bool,
    latex_math_buffer: String,

    // Syntax highlighter (lazy initialized)
    highlighter: Option<SyntaxHighlighter>,

    // Position tracking for selection support
    formatting_stack: Vec<FormatMark>,
    position_map: PositionMap,
    current_render_col: usize,
}

impl<'a> RendererState<'a> {
    fn new(theme: &'a Theme, options: &'a RenderOptions) -> Self {
        // Create highlighter if syntax highlighting is enabled
        let highlighter = if options.syntax_highlighting {
            Some(SyntaxHighlighter::new().theme(&options.syntax_theme))
        } else {
            None
        };

        Self {
            theme,
            options,
            lines: Vec::new(),
            current_spans: Vec::new(),
            links: Vec::new(),
            headings: Vec::new(),
            style_stack: vec![theme.text],
            in_heading: None,
            in_blockquote: false,
            in_code_block: false,
            code_block_lang: String::new(),
            code_block_content: String::new(),
            in_list: false,
            list_depth: 0,
            list_counters: Vec::new(),
            list_is_ordered: Vec::new(),
            current_task_state: None,
            in_table: false,
            table_columns: 0,
            table_alignments: Vec::new(),
            table_rows: Vec::new(),
            current_table_row: Vec::new(),
            current_table_cell: Vec::new(),
            in_table_header: false,
            current_link: None,
            current_link_text: String::new(),
            pending_newline: false,
            needs_list_prefix: false,
            in_latex_math: false,
            latex_math_buffer: String::new(),
            highlighter,
            // Position tracking
            formatting_stack: Vec::new(),
            position_map: PositionMap::new(),
            current_render_col: 0,
        }
    }

    fn current_style(&self) -> Style {
        self.style_stack.last().copied().unwrap_or(self.theme.text)
    }

    fn push_style(&mut self, style: Style) {
        // Merge with current style
        let current = self.current_style();
        let merged = current.patch(style);
        self.style_stack.push(merged);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn push_text(&mut self, text: &str) {
        use unicode_segmentation::UnicodeSegmentation;

        if text.is_empty() {
            return;
        }

        if self.in_table {
            self.current_table_cell
                .push(RSpan::styled(text.to_string(), self.current_style()));
            return;
        }

        // Buffer code block content for syntax highlighting
        if self.in_code_block && self.highlighter.is_some() {
            self.code_block_content.push_str(text);
            return;
        }

        // Add list prefix if needed (for tight lists without paragraph wrappers)
        if self.needs_list_prefix && self.in_list && self.list_depth > 0 {
            self.needs_list_prefix = false;
            let prefix = self.get_list_prefix();
            if !prefix.is_empty() {
                let prefix_len = prefix.graphemes(true).count();
                let style = if self.list_is_ordered.last().copied().unwrap_or(false) {
                    self.theme.list_number
                } else {
                    self.theme.list_bullet
                };
                self.current_spans.push(RSpan::styled(prefix, style));
                // Advance render column past prefix (prefix has no formatting stack)
                self.current_render_col += prefix_len;
            }
        }

        // Track link text
        if self.current_link.is_some() {
            self.current_link_text.push_str(text);
        }

        // Handle embedded newlines - split into separate lines
        // This is especially important for code blocks where content may contain \n
        if text.contains('\n') {
            let mut lines_iter = text.split('\n').peekable();
            while let Some(line) = lines_iter.next() {
                if !line.is_empty() {
                    // Track position for each grapheme if position tracking enabled
                    if self.options.track_positions {
                        for _grapheme in line.graphemes(true) {
                            if let Some(line_map) = self.position_map.current_line_mut() {
                                line_map.push(CharMapping::new(
                                    self.current_render_col,
                                    self.formatting_stack.clone(),
                                ));
                            }
                            self.current_render_col += 1;
                        }
                    }
                    self.current_spans
                        .push(RSpan::styled(line.to_string(), self.current_style()));
                }
                // If there's another line after this, finish the current line
                if lines_iter.peek().is_some() {
                    self.finish_line();
                }
            }
        } else {
            // Track position for each grapheme if position tracking enabled
            if self.options.track_positions {
                for _grapheme in text.graphemes(true) {
                    if let Some(line_map) = self.position_map.current_line_mut() {
                        line_map.push(CharMapping::new(
                            self.current_render_col,
                            self.formatting_stack.clone(),
                        ));
                    }
                    self.current_render_col += 1;
                }
            }
            self.current_spans
                .push(RSpan::styled(text.to_string(), self.current_style()));
        }
    }

    fn finish_line(&mut self) {
        if self.in_table {
            return;
        }

        let mut spans = std::mem::take(&mut self.current_spans);

        // Add blockquote prefix if needed
        let prefix = if self.in_blockquote && !spans.is_empty() {
            Some(RSpan::styled(
                self.theme.blockquote_prefix.to_string(),
                self.theme.blockquote_marker,
            ))
        } else {
            None
        };

        if !spans.is_empty() || self.pending_newline {
            // Apply word wrapping if width is set and we're not in a code block
            if self.options.width > 0 && !self.in_code_block {
                let wrapped = self.wrap_spans(spans, prefix.clone());
                for line in wrapped {
                    self.lines.push(line);
                }
            } else {
                if let Some(p) = prefix {
                    spans.insert(0, p);
                }
                self.lines.push(Line::from(spans));
            }
            self.pending_newline = false;
        }

        // Reset position tracking for next line
        if self.options.track_positions {
            self.current_render_col = 0;
            self.position_map.start_line();
        }
    }

    /// Wrap spans to fit within the configured width.
    fn wrap_spans(&self, spans: Vec<RSpan<'static>>, prefix: Option<RSpan<'static>>) -> Vec<Line<'static>> {
        let max_width = self.options.width;
        let prefix_len = prefix.as_ref().map(|p| p.content.width()).unwrap_or(0);
        let effective_width = max_width.saturating_sub(prefix_len);

        if effective_width == 0 {
            let mut result = spans;
            if let Some(p) = prefix {
                result.insert(0, p);
            }
            return vec![Line::from(result)];
        }

        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut current_line: Vec<RSpan<'static>> = Vec::new();
        let mut current_width = 0usize;

        for span in spans {
            let style = span.style;
            let text = span.content.to_string();

            // Split by words
            let mut remaining = text.as_str();
            while !remaining.is_empty() {
                // Find next word boundary
                let trimmed = remaining.trim_start();
                let leading_space = remaining.len() - trimmed.len();

                if leading_space > 0 {
                    if current_width == 0 && lines.is_empty() && current_line.is_empty() {
                        // Preserve leading whitespace at the very start (e.g., list indent)
                        let indent_str = " ".repeat(leading_space);
                        current_line.push(RSpan::styled(indent_str, style));
                        current_width += leading_space;
                    } else if current_width > 0 {
                        // Between words - add single space
                        if current_width < effective_width {
                            current_line.push(RSpan::styled(" ".to_string(), style));
                            current_width += 1;
                        }
                    }
                }

                remaining = trimmed;
                if remaining.is_empty() {
                    break;
                }

                // Find end of word
                let word_end = remaining.find(char::is_whitespace).unwrap_or(remaining.len());
                let word = &remaining[..word_end];
                remaining = &remaining[word_end..];

                // Check if word fits on current line
                if current_width + word.width() > effective_width && current_width > 0 {
                    // Start new line
                    let mut line_spans = std::mem::take(&mut current_line);
                    if let Some(ref p) = prefix {
                        line_spans.insert(0, p.clone());
                    }
                    lines.push(Line::from(line_spans));
                    current_width = 0;
                }

                // Add word (possibly to new line)
                if !word.is_empty() {
                    current_line.push(RSpan::styled(word.to_string(), style));
                    current_width += word.width();
                }
            }
        }

        // Add remaining content
        if !current_line.is_empty() || lines.is_empty() {
            if let Some(p) = prefix {
                current_line.insert(0, p);
            }
            lines.push(Line::from(current_line));
        }

        lines
    }

    fn add_blank_line(&mut self) {
        self.finish_line();
        self.lines.push(Line::from(vec![]));
    }

    fn get_list_prefix(&mut self) -> String {
        let indent = " ".repeat(self.list_depth.saturating_sub(1) * self.theme.list_indent);

        // Handle task lists
        if let Some(task_state) = self.current_task_state.take() {
            let marker = match task_state {
                TaskState::Checked => self.theme.task_checked_char,
                TaskState::Unchecked => self.theme.task_unchecked_char,
                TaskState::NotTask => self.theme.bullet_char,
            };
            return format!("{}{} ", indent, marker);
        }

        if self.list_depth == 0 {
            return String::new();
        }

        let idx = self.list_depth - 1;
        if idx < self.list_is_ordered.len() && self.list_is_ordered[idx] {
            let num = self.list_counters.get(idx).copied().unwrap_or(1);
            format!("{}{}. ", indent, num)
        } else {
            format!("{}{} ", indent, self.theme.bullet_char)
        }
    }

    fn render_horizontal_rule(&mut self) {
        let width = if self.options.width > 0 {
            self.options.width
        } else {
            40
        };
        let hr = self.theme.hr_char.to_string().repeat(width);
        self.lines
            .push(Line::from(vec![RSpan::styled(hr, self.theme.horizontal_rule)]));
    }

    fn render_table(&mut self) {
        if self.table_rows.is_empty() {
            return;
        }

        // Calculate column widths
        let mut col_widths: Vec<usize> = vec![0; self.table_columns];
        for row in &self.table_rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_widths.len() {
                    let cell_width: usize = cell.iter().map(|s| s.content.width()).sum();
                    col_widths[i] = col_widths[i].max(cell_width);
                }
            }
        }

        // Ensure minimum width
        for w in &mut col_widths {
            *w = (*w).max(3);
        }

        // Render top border
        let top_border: String = col_widths
            .iter()
            .map(|w| "─".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("┬");
        self.lines.push(Line::from(vec![RSpan::styled(
            format!("┌{}┐", top_border),
            self.theme.table_border,
        )]));

        // Render rows
        for (row_idx, row) in self.table_rows.iter().enumerate() {
            let mut line_spans = vec![RSpan::styled("│ ".to_string(), self.theme.table_border)];

            for (col_idx, cell) in row.iter().enumerate() {
                let cell_text: String = cell.iter().map(|s| s.content.to_string()).collect();
                let width = col_widths.get(col_idx).copied().unwrap_or(3);
                let align = self.table_alignments.get(col_idx).copied().unwrap_or(Alignment::Default);

                let text_width = cell_text.width();
                let pad = width.saturating_sub(text_width);
                let padded = match align {
                    Alignment::Center => {
                        let left = pad / 2;
                        let right = pad - left;
                        format!("{}{}{}", " ".repeat(left), cell_text, " ".repeat(right))
                    }
                    Alignment::Right => format!("{}{}", " ".repeat(pad), cell_text),
                    _ => format!("{}{}", cell_text, " ".repeat(pad)),
                };

                let style = if row_idx == 0 {
                    self.theme.table_header
                } else {
                    self.theme.table_cell
                };

                line_spans.push(RSpan::styled(padded, style));
                line_spans.push(RSpan::styled(" │ ".to_string(), self.theme.table_border));
            }

            self.lines.push(Line::from(line_spans));

            // Add separator after every row except the last
            if row_idx < self.table_rows.len() - 1 {
                let sep: String = col_widths
                    .iter()
                    .enumerate()
                    .map(|(i, w)| {
                        // For header separator (row_idx == 0), show alignment markers
                        // For data row separators, use plain dashes
                        if row_idx == 0 {
                            let align = self.table_alignments.get(i).copied().unwrap_or(Alignment::Default);
                            match align {
                                Alignment::Left => format!(":{}─", "─".repeat(*w)),
                                Alignment::Right => format!("{}─:", "─".repeat(*w)),
                                Alignment::Center => format!(":{}:", "─".repeat(*w)),
                                _ => "─".repeat(*w + 2),
                            }
                        } else {
                            // Data row separators: plain dashes
                            "─".repeat(*w + 2)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("┼");
                self.lines.push(Line::from(vec![RSpan::styled(
                    format!("├{}┤", sep),
                    self.theme.table_border,
                )]));
            }
        }

        // Render bottom border
        let bottom_border: String = col_widths
            .iter()
            .map(|w| "─".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("┴");
        self.lines.push(Line::from(vec![RSpan::styled(
            format!("└{}┘", bottom_border),
            self.theme.table_border,
        )]));

        // Clear table state
        self.table_rows.clear();
        self.table_columns = 0;
        self.table_alignments.clear();
    }
}

impl ParserHandler for RendererState<'_> {
    fn enter_block(&mut self, block: Block) -> bool {
        match block {
            Block::Document => {}

            Block::Paragraph => {
                // Add list prefix at start of list item paragraph (for loose lists)
                if self.needs_list_prefix && self.in_list && self.list_depth > 0 {
                    self.needs_list_prefix = false;
                    let prefix = self.get_list_prefix();
                    if !prefix.is_empty() {
                        let style = if self.list_is_ordered.last().copied().unwrap_or(false) {
                            self.theme.list_number
                        } else {
                            self.theme.list_bullet
                        };
                        self.current_spans.push(RSpan::styled(prefix, style));
                    }
                }
            }

            Block::Heading(HeadingDetail { level }) => {
                self.in_heading = Some(level);
                self.push_style(self.theme.heading_style(level));

                // Add heading prefix (optional)
                let prefix = "#".repeat(level as usize);
                self.current_spans.push(RSpan::styled(
                    format!("{} ", prefix),
                    self.theme.heading_style(level),
                ));
            }

            Block::Quote => {
                self.in_blockquote = true;
                self.push_style(self.theme.blockquote);
            }

            Block::Code(CodeBlockDetail { lang, .. }) => {
                self.in_code_block = true;

                // Show language label if present
                if !lang.is_empty() {
                    self.lines.push(Line::from(vec![RSpan::styled(
                        format!("{}:", lang),
                        self.theme.code_block_info,
                    )]));
                }

                self.code_block_lang = lang;
                self.push_style(self.theme.code_block);
            }

            Block::UnorderedList(UnorderedListDetail { .. }) => {
                // Finish current line before starting nested list
                if self.list_depth > 0 {
                    self.finish_line();
                }
                self.in_list = true;
                self.list_depth += 1;
                self.list_is_ordered.push(false);
                self.list_counters.push(1);
            }

            Block::OrderedList(OrderedListDetail { start, .. }) => {
                // Finish current line before starting nested list
                if self.list_depth > 0 {
                    self.finish_line();
                }
                self.in_list = true;
                self.list_depth += 1;
                self.list_is_ordered.push(true);
                self.list_counters.push(start);
            }

            Block::ListItem(ListItemDetail { task_state }) => {
                if task_state != TaskState::NotTask {
                    self.current_task_state = Some(task_state);
                }
                // Mark that we need to add a list prefix on the next text/paragraph
                self.needs_list_prefix = true;
            }

            Block::HorizontalRule => {
                self.render_horizontal_rule();
            }

            Block::Html => {
                self.push_style(self.theme.raw_html);
            }

            Block::Table(TableDetail { column_count, .. }) => {
                self.in_table = true;
                self.table_columns = column_count as usize;
                self.table_alignments = vec![Alignment::Default; column_count as usize];
            }

            Block::TableHead => {
                self.in_table_header = true;
            }

            Block::TableBody => {
                self.in_table_header = false;
            }

            Block::TableRow => {
                self.current_table_row = Vec::new();
            }

            Block::TableHeaderCell(TableCellDetail { alignment }) | Block::TableCell(TableCellDetail { alignment }) => {
                self.current_table_cell = Vec::new();
                // Store alignment
                let col_idx = self.current_table_row.len();
                if col_idx < self.table_alignments.len() {
                    self.table_alignments[col_idx] = alignment;
                }
            }

            _ => {}
        }
        true
    }

    fn leave_block(&mut self, block_type: BlockType) -> bool {
        match block_type {
            BlockType::Document => {}

            BlockType::Paragraph => {
                self.finish_line();
                if self.options.paragraph_space && !self.in_list {
                    self.add_blank_line();
                }
            }

            BlockType::Heading => {
                // Record heading info
                if let Some(level) = self.in_heading.take() {
                    let text: String = self.current_spans.iter().map(|s| s.content.to_string()).collect();
                    self.headings.push(HeadingInfo {
                        line: self.lines.len(),
                        level,
                        text: text.trim_start_matches(['#', ' ']).to_string(),
                    });
                }
                self.finish_line();
                self.pop_style();
                if self.options.heading_space {
                    self.add_blank_line();
                }
            }

            BlockType::Quote => {
                self.finish_line();
                self.in_blockquote = false;
                self.pop_style();
                self.add_blank_line();
            }

            BlockType::Code => {
                // Apply syntax highlighting if enabled and content was buffered
                if let Some(ref highlighter) = self.highlighter {
                    if !self.code_block_content.is_empty() {
                        // Check thread-local cache for previously highlighted code
                        let hash = hash_code_block(&self.code_block_content, &self.code_block_lang);

                        let highlighted_lines = HIGHLIGHT_CACHE.with(|cache| {
                            let mut cache = cache.borrow_mut();
                            if let Some(cached) = cache.get(&hash) {
                                // Cache hit - reuse previously highlighted lines
                                return cached.clone();
                            }
                            // Cache miss - highlight and store
                            let lines = highlighter.highlight(&self.code_block_content, &self.code_block_lang);
                            if cache.len() >= HIGHLIGHT_CACHE_CAP {
                                cache.clear();
                            }
                            cache.insert(hash, lines.clone());
                            lines
                        });

                        self.lines.extend(highlighted_lines);
                        self.code_block_content.clear();
                    }
                } else {
                    self.finish_line();
                }
                self.in_code_block = false;
                self.code_block_lang.clear();
                self.pop_style();
                if self.options.code_block_space {
                    self.add_blank_line();
                }
            }

            BlockType::UnorderedList | BlockType::OrderedList => {
                self.list_depth = self.list_depth.saturating_sub(1);
                self.list_is_ordered.pop();
                self.list_counters.pop();
                if self.list_depth == 0 {
                    self.in_list = false;
                    if self.options.list_space {
                        self.add_blank_line();
                    }
                }
            }

            BlockType::ListItem => {
                self.finish_line();
                // Increment counter for ordered lists
                if let Some(counter) = self.list_counters.last_mut() {
                    *counter += 1;
                }
            }

            BlockType::HorizontalRule => {
                self.add_blank_line();
            }

            BlockType::Html => {
                self.finish_line();
                self.pop_style();
            }

            BlockType::Table => {
                self.render_table();
                self.in_table = false;
                self.add_blank_line();
            }

            BlockType::TableHead | BlockType::TableBody => {}

            BlockType::TableRow => {
                self.table_rows.push(std::mem::take(&mut self.current_table_row));
            }

            BlockType::TableHeaderCell | BlockType::TableCell => {
                self.current_table_row.push(std::mem::take(&mut self.current_table_cell));
            }

            // Handle any future variants
            _ => {}
        }
        true
    }

    fn enter_span(&mut self, span: Span) -> bool {
        match span {
            Span::Emphasis => {
                self.push_style(self.theme.emphasis);
                if self.options.track_positions {
                    self.formatting_stack.push(FormatMark::Italic);
                }
            }
            Span::Strong => {
                self.push_style(self.theme.strong);
                if self.options.track_positions {
                    self.formatting_stack.push(FormatMark::Bold);
                }
            }
            Span::Strikethrough => {
                self.push_style(self.theme.strikethrough);
                if self.options.track_positions {
                    self.formatting_stack.push(FormatMark::Strike);
                }
            }
            Span::Underline => {
                self.push_style(self.theme.underline);
                if self.options.track_positions {
                    self.formatting_stack.push(FormatMark::Underline);
                }
            }
            Span::Code => {
                self.push_style(self.theme.code_inline);
                if self.options.track_positions {
                    self.formatting_stack.push(FormatMark::Code);
                }
            }
            Span::Link(detail) => {
                self.current_link = Some(detail);
                self.current_link_text.clear();
                self.push_style(self.theme.link);
                if self.options.track_positions {
                    self.formatting_stack.push(FormatMark::Link);
                }
            }
            Span::Image(ImageDetail { src, title }) => {
                self.push_style(self.theme.image);
                // Render as [alt](src)
                let alt_text = if title.is_empty() { "image" } else { &title };
                self.push_text(&format!("[{}]", alt_text));
                if !src.is_empty() {
                    self.current_spans
                        .push(RSpan::styled(format!("({})", src), self.theme.link_url));
                }
            }
            Span::LatexMath | Span::LatexMathDisplay => {
                self.push_style(self.theme.latex_math);
                self.in_latex_math = true;
                self.latex_math_buffer.clear();
                if self.options.track_positions {
                    self.formatting_stack.push(FormatMark::Math);
                }
            }
            Span::WikiLink(WikiLinkDetail { target }) => {
                self.push_style(self.theme.wiki_link);
                if self.options.track_positions {
                    self.formatting_stack.push(FormatMark::Link);
                }
                // Store as link
                self.links.push(LinkInfo {
                    line: self.lines.len(),
                    url: target.clone(),
                    text: target,
                    is_autolink: false,
                });
            }

            _ => {}
        }
        true
    }

    fn leave_span(&mut self, span_type: SpanType) -> bool {
        match span_type {
            SpanType::Link => {
                // Record link info
                if let Some(detail) = self.current_link.take() {
                    self.links.push(LinkInfo {
                        line: self.lines.len(),
                        url: detail.href.clone(),
                        text: std::mem::take(&mut self.current_link_text),
                        is_autolink: detail.is_autolink,
                    });

                    // Optionally show URL
                    if self.theme.show_link_urls && !detail.href.is_empty() {
                        self.pop_style();
                        if self.options.track_positions {
                            self.formatting_stack.pop();
                        }
                        self.current_spans
                            .push(RSpan::styled(format!(" ({})", detail.href), self.theme.link_url));
                        return true;
                    }
                }
                self.pop_style();
                if self.options.track_positions {
                    self.formatting_stack.pop();
                }
            }
            SpanType::Image => {
                self.pop_style();
                // No formatting mark was pushed for images
            }
            SpanType::Emphasis => {
                self.pop_style();
                if self.options.track_positions {
                    self.formatting_stack.pop();
                }
            }
            SpanType::Strong => {
                self.pop_style();
                if self.options.track_positions {
                    self.formatting_stack.pop();
                }
            }
            SpanType::Strikethrough => {
                self.pop_style();
                if self.options.track_positions {
                    self.formatting_stack.pop();
                }
            }
            SpanType::Underline => {
                self.pop_style();
                if self.options.track_positions {
                    self.formatting_stack.pop();
                }
            }
            SpanType::Code => {
                self.pop_style();
                if self.options.track_positions {
                    self.formatting_stack.pop();
                }
            }
            SpanType::WikiLink => {
                self.pop_style();
                if self.options.track_positions {
                    self.formatting_stack.pop();
                }
            }
            SpanType::LatexMath | SpanType::LatexMathDisplay => {
                if !self.latex_math_buffer.is_empty() {
                    let buf = std::mem::take(&mut self.latex_math_buffer);
                    let converted = crate::latex::latex_to_unicode(&buf);
                    self.push_text(&converted);
                }
                self.in_latex_math = false;
                self.pop_style();
                if self.options.track_positions {
                    self.formatting_stack.pop();
                }
            }

            // Handle any future variants
            _ => {
                self.pop_style();
            }
        }
        true
    }

    fn text(&mut self, text_type: TextType, text: &str) -> bool {
        match text_type {
            TextType::Normal | TextType::Code => {
                self.push_text(text);
            }
            TextType::LatexMath => {
                self.latex_math_buffer.push_str(text);
            }
            TextType::HardBreak => {
                self.finish_line();
                // Add indent for continued list items
                if self.in_list && self.list_depth > 0 {
                    let indent = " ".repeat(self.list_depth * self.theme.list_indent);
                    self.current_spans.push(RSpan::raw(indent));
                }
            }
            TextType::SoftBreak => {
                if self.options.hard_breaks {
                    // Treat as line break
                    self.finish_line();
                    // Add indent for continued list items
                    if self.in_list && self.list_depth > 0 {
                        let indent = " ".repeat(self.list_depth * self.theme.list_indent);
                        self.current_spans.push(RSpan::raw(indent));
                    }
                } else {
                    // Standard markdown: treat as space
                    self.push_text(" ");
                }
            }
            TextType::Entity => {
                // Render entity literally (could decode common ones)
                self.current_spans
                    .push(RSpan::styled(text.to_string(), self.theme.html_entity));
            }
            TextType::Html => {
                self.current_spans
                    .push(RSpan::styled(text.to_string(), self.theme.raw_html));
            }
            TextType::NullChar => {
                self.push_text("\u{FFFD}");
            }

            // Handle any future variants
            _ => {
                self.push_text(text);
            }
        }
        true
    }
}

/// Render markdown to ratatui Text.
///
/// # Arguments
/// * `markdown` - The markdown source text
/// * `theme` - The theme to use for styling
/// * `options` - Rendering options
///
/// # Returns
/// A `RenderedMarkdown` containing the styled text and metadata.
///
/// # Example
///
/// ```
/// use ratatui_md::{render, Theme, RenderOptions};
///
/// let markdown = "# Hello\n\nThis is **bold** text.";
/// let result = render(markdown, &Theme::default(), &RenderOptions::default());
/// ```
pub fn render(
    markdown: &str,
    theme: &Theme,
    options: &RenderOptions,
) -> RenderedMarkdown {
    let mut state = RendererState::new(theme, options);

    // Initialize first line for position tracking
    if options.track_positions {
        state.position_map.start_line();
    }

    // Parse and render
    let _ = parse(markdown, options.parser_flags, &mut state);

    // Ensure last line is finished
    state.finish_line();

    let line_count = state.lines.len();

    // Extract position map if tracking was enabled
    let position_map = if options.track_positions {
        Some(state.position_map)
    } else {
        None
    };

    RenderedMarkdown {
        text: Text::from(state.lines),
        links: state.links,
        headings: state.headings,
        line_count,
        position_map,
    }
}

/// Render markdown to ratatui Text with default options.
///
/// Convenience function using default theme and options.
pub fn render_default(markdown: &str) -> Text<'static> {
    render(markdown, &Theme::default(), &RenderOptions::default()).text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_rendering() {
        let result = render("Hello **world**", &Theme::default(), &RenderOptions::default());
        assert!(!result.text.lines.is_empty());
    }

    #[test]
    fn test_heading() {
        let result = render("# Title", &Theme::default(), &RenderOptions::default());
        assert_eq!(result.headings.len(), 1);
        assert_eq!(result.headings[0].level, 1);
        assert_eq!(result.headings[0].text, "Title");
    }

    #[test]
    fn test_links() {
        let result = render(
            "[click me](https://example.com)",
            &Theme::default(),
            &RenderOptions::default(),
        );
        assert_eq!(result.links.len(), 1);
        assert_eq!(result.links[0].url, "https://example.com");
        assert_eq!(result.links[0].text, "click me");
    }

    #[test]
    fn test_list() {
        let result = render("- item 1\n- item 2", &Theme::default(), &RenderOptions::default());
        assert!(result.text.lines.len() >= 2);
    }

    #[test]
    fn test_code_block() {
        let result = render("```rust\nfn main() {}\n```", &Theme::default(), &RenderOptions::default());
        assert!(result.text.lines.len() >= 1);
    }

    #[test]
    fn test_table() {
        let result = render(
            "| A | B |\n|---|---|\n| 1 | 2 |",
            &Theme::default(),
            &RenderOptions::github(),
        );
        // Table renders with borders
        assert!(result.text.lines.len() >= 3);
    }

    #[test]
    fn test_table_row_separators() {
        // Test that tables render with separators between all rows (not just header)
        let markdown = "| Name | Age | Status |\n|------|-----|--------|\n| Alice | 30 | Active |\n| Bob | 25 | Inactive |";
        let result = render(
            markdown,
            &Theme::default(),
            &RenderOptions::github(),
        );
        
        // Convert rendered lines to text for inspection
        let rendered_text: Vec<String> = result
            .text
            .lines
            .iter()
            .map(|line| line.spans.iter().map(|s| s.content.to_string()).collect())
            .collect();
        
        // Should have: top border + header + header separator + row1 + row separator + row2 + bottom border = 7 lines
        assert!(rendered_text.len() >= 7, "Table should have at least 7 lines (top, header, h-sep, row1, row-sep, row2, bottom), got {}", rendered_text.len());
        
        // Verify separators exist (lines containing ├)
        let separator_count = rendered_text.iter().filter(|l| l.contains("├")).count();
        assert_eq!(separator_count, 2, "Should have 2 separators (after header and after first data row), got {}", separator_count);
        
        // Verify both separators use ┼ for column junctions
        for line in &rendered_text {
            if line.contains("├") {
                assert!(line.contains("┼"), "Separator line should contain column junctions (┼): {}", line);
            }
        }
    }
}

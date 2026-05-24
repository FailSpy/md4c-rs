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
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use unicode_width::UnicodeWidthStr;

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

/// Strategy for rendering tables that exceed the viewport width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableMode {
    /// Shrink column widths proportionally to fit the viewport, word-wrap
    /// each cell within its column. Overflow beyond min column widths falls
    /// back to truncation with a `▶` indicator.
    #[default]
    SqueezeWrap,
    /// Render at natural column widths and clip lines that exceed the
    /// viewport, marking truncation with a trailing `▶`.
    Truncate,
    /// Render at natural column widths with no viewport-aware adjustment.
    Natural,
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
    /// How to render tables when they exceed `width`. Ignored when `width == 0`.
    pub table_mode: TableMode,
    /// Minimum column width when squeezing tables (in display cells).
    pub min_column_width: usize,
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
            table_mode: TableMode::default(),
            min_column_width: 4,
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

    /// Set the table rendering strategy. See [`TableMode`].
    pub fn with_table_mode(mut self, mode: TableMode) -> Self {
        self.table_mode = mode;
        self
    }

    /// Set the minimum column width when squeezing tables.
    pub fn with_min_column_width(mut self, width: usize) -> Self {
        self.min_column_width = width.max(1);
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

/// Word-wrap a sequence of styled spans to `effective_width` cells.
///
/// Grapheme-cluster aware: word boundaries fall ONLY on grapheme-cluster
/// boundaries (never mid-ZWJ-sequence, never mid-combining-mark). Display
/// widths use `unicode-width` per grapheme cluster, not per code point —
/// so a CJK ideograph counts as 2 cells, a ZWJ family emoji counts as
/// its measured width, etc.
///
/// Preserves per-span style. Breaks at whitespace between words. Single
/// words longer than `effective_width` overflow their line (never
/// dropped). Returns at least one line (possibly empty) to match the
/// contract of higher-level wrappers.
fn wrap_spans_to_width(
    spans: &[RSpan<'static>],
    effective_width: usize,
) -> Vec<Vec<RSpan<'static>>> {
    use unicode_segmentation::UnicodeSegmentation;

    if effective_width == 0 {
        return vec![spans.to_vec()];
    }

    /// Display width of a grapheme cluster (sum of code-point widths).
    /// Combining marks and ZWJ contribute 0; CJK ideographs contribute 2.
    #[inline]
    fn cluster_width(g: &str) -> usize {
        g.width()
    }

    /// True if the cluster is whitespace-only (used as word boundary).
    /// A cluster is whitespace iff every code point in it is whitespace.
    #[inline]
    fn cluster_is_whitespace(g: &str) -> bool {
        !g.is_empty() && g.chars().all(char::is_whitespace)
    }

    let mut lines: Vec<Vec<RSpan<'static>>> = Vec::new();
    let mut current_line: Vec<RSpan<'static>> = Vec::new();
    let mut current_width = 0usize;

    for span in spans {
        let style = span.style;
        let text = span.content.as_ref();

        // Walk the span as a sequence of grapheme clusters. We assemble
        // each "word" (run of non-whitespace clusters) and each "gap" (run
        // of whitespace clusters) so that wrap decisions land on cluster
        // boundaries — splitting mid-ZWJ would corrupt the rendered glyph.
        let clusters: Vec<&str> = text.graphemes(true).collect();
        let mut idx = 0usize;
        while idx < clusters.len() {
            // Eat leading whitespace clusters.
            let ws_start = idx;
            while idx < clusters.len() && cluster_is_whitespace(clusters[idx]) {
                idx += 1;
            }
            let leading_ws_width: usize =
                clusters[ws_start..idx].iter().map(|g| cluster_width(g)).sum();

            if leading_ws_width > 0 {
                if current_width == 0 && lines.is_empty() && current_line.is_empty() {
                    // Preserve leading whitespace at the very start (e.g., list indent).
                    let indent_str: String = clusters[ws_start..idx].concat();
                    current_line.push(RSpan::styled(indent_str, style));
                    current_width += leading_ws_width;
                } else if current_width > 0 && current_width < effective_width {
                    // Collapse run of inter-word whitespace to a single space cell.
                    current_line.push(RSpan::styled(" ".to_string(), style));
                    current_width += 1;
                }
            }

            if idx >= clusters.len() {
                break;
            }

            // Eat a "word" = run of non-whitespace clusters.
            let word_start = idx;
            while idx < clusters.len() && !cluster_is_whitespace(clusters[idx]) {
                idx += 1;
            }
            let word_clusters = &clusters[word_start..idx];
            if word_clusters.is_empty() {
                continue;
            }
            let word_str: String = word_clusters.concat();
            let word_width: usize = word_clusters.iter().map(|g| cluster_width(g)).sum();

            // Wrap before pushing the word if it would overflow AND the
            // current line has at least one word already (single overlong
            // words still overflow their line — never dropped).
            if current_width + word_width > effective_width && current_width > 0 {
                lines.push(std::mem::take(&mut current_line));
                current_width = 0;
            }

            current_line.push(RSpan::styled(word_str, style));
            current_width += word_width;
        }
    }

    if !current_line.is_empty() || lines.is_empty() {
        lines.push(current_line);
    }

    lines
}

/// Clip a line to `max_width` display cells, appending a `▶` truncation
/// marker when clipping occurs. Preserves the style of the last visible span
/// for the marker.
fn truncate_line_to_width(line: Line<'static>, max_width: usize) -> Line<'static> {
    if max_width == 0 {
        return line;
    }

    let total: usize = line.spans.iter().map(|s| s.content.width()).sum();
    if total <= max_width {
        return line;
    }

    let marker = "▶";
    let marker_w = marker.width();
    let budget = max_width.saturating_sub(marker_w);

    let mut out: Vec<RSpan<'static>> = Vec::new();
    let mut used = 0usize;
    let mut last_style: Option<Style> = None;

    for span in line.spans {
        let style = span.style;
        last_style = Some(style);
        let w = span.content.width();
        if used + w <= budget {
            out.push(span);
            used += w;
        } else {
            let mut acc = String::new();
            let mut acc_w = 0usize;
            let remaining = budget.saturating_sub(used);
            for ch in span.content.chars() {
                let ch_w = ch.to_string().width();
                if acc_w + ch_w > remaining {
                    break;
                }
                acc.push(ch);
                acc_w += ch_w;
            }
            if !acc.is_empty() {
                out.push(RSpan::styled(acc, style));
            }
            break;
        }
    }

    let style = last_style.unwrap_or_default();
    out.push(RSpan::styled(marker.to_string(), style));
    Line::from(out)
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
    /// Byte offset in the input source for the *current text run* being
    /// pushed via push_text. Set by `ParserHandler::text` from
    /// `TextContext::source_offset`. `None` when MD4C delivered the run
    /// from a scratch buffer (entity decode, normalization). Per-grapheme
    /// spans within the run are derived from this base offset + the
    /// grapheme's byte offset inside the run.
    current_source_offset: Option<u32>,
    /// Semantic role of the current text run. Stack-managed by
    /// enter_block/leave_block/enter_span/leave_span. Defaults to
    /// PlainText when the stack is empty.
    source_kind_stack: Vec<cadenza_anchor::SourceKind>,
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
            current_source_offset: None,
            source_kind_stack: Vec::new(),
        }
    }

    #[inline]
    fn current_source_kind(&self) -> cadenza_anchor::SourceKind {
        self.source_kind_stack
            .last()
            .copied()
            .unwrap_or(cadenza_anchor::SourceKind::PlainText)
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
                let style = if self.list_is_ordered.last().copied().unwrap_or(false) {
                    self.theme.list_number
                } else {
                    self.theme.list_bullet
                };
                // Push the rendered prefix span first (so render output is unchanged).
                self.current_spans.push(RSpan::styled(prefix.clone(), style));
                // Then push matching decorative position mappings (no source,
                // DecorativeKind::ListBullet). push_decorative_position_mappings
                // advances current_render_col by one per grapheme, replacing the
                // raw `current_render_col += prefix_len` of the previous code.
                self.push_decorative_position_mappings(
                    &prefix,
                    cadenza_anchor::DecorativeKind::ListBullet,
                );
            }
        }

        // Track link text
        if self.current_link.is_some() {
            self.current_link_text.push_str(text);
        }

        // Handle embedded newlines - split into separate lines
        // This is especially important for code blocks where content may contain \n
        if text.contains('\n') {
            // Track byte offset within the original `text` so each rendered
            // line's source spans index correctly back into the input even
            // when the renderer splits on embedded newlines.
            let mut byte_cursor: usize = 0;
            let mut lines_iter = text.split('\n').peekable();
            while let Some(line) = lines_iter.next() {
                let line_byte_start = byte_cursor;
                if !line.is_empty() {
                    if self.options.track_positions {
                        self.push_text_position_mappings(line, line_byte_start);
                    }
                    self.current_spans
                        .push(RSpan::styled(line.to_string(), self.current_style()));
                }
                byte_cursor += line.len();
                if lines_iter.peek().is_some() {
                    byte_cursor += 1; // account for the '\n' separator
                    self.finish_line();
                }
            }
        } else {
            if self.options.track_positions {
                self.push_text_position_mappings(text, 0);
            }
            self.current_spans
                .push(RSpan::styled(text.to_string(), self.current_style()));
        }
    }

    /// Push per-grapheme CharMappings for `text`, where `text_byte_start_in_run`
    /// is the byte offset of `text` within the current MD4C text run (zero for
    /// non-split runs; cumulative byte position for newline-split runs).
    ///
    /// Source spans index into the input by composing
    /// `current_source_offset + text_byte_start_in_run + grapheme_byte_offset`.
    /// When `current_source_offset` is `None` (scratch-buffer run), spans are
    /// left as `None` and the consumer's delimiter-walk fallback handles
    /// recovery.
    fn push_text_position_mappings(&mut self, text: &str, text_byte_start_in_run: usize) {
        use unicode_segmentation::UnicodeSegmentation;

        let source_kind = self.current_source_kind();
        let base_offset = self.current_source_offset;

        for (grapheme_byte_start, grapheme) in text.grapheme_indices(true) {
            let source = base_offset.and_then(|base| {
                let abs_start = (base as usize)
                    .checked_add(text_byte_start_in_run)?
                    .checked_add(grapheme_byte_start)?;
                let abs_end = abs_start.checked_add(grapheme.len())?;
                if abs_start <= u32::MAX as usize && abs_end <= u32::MAX as usize {
                    Some(cadenza_anchor::SourceSpan::new(
                        abs_start as u32,
                        abs_end as u32,
                    ))
                } else {
                    None
                }
            });

            if let Some(line_map) = self.position_map.current_line_mut() {
                line_map.push(CharMapping::new_kinded(
                    self.current_render_col,
                    self.formatting_stack.clone(),
                    source,
                    source_kind,
                    None,
                ));
            }
            self.current_render_col += 1;
        }
    }

    /// Push CharMappings for a decorative span (list bullets, blockquote bars,
    /// heading markers, table borders, etc.). Each grapheme of `text` gets
    /// `source = None`, `decorative = Some(kind)`. Caller is responsible for
    /// emitting the matching `RSpan` into `current_spans`.
    fn push_decorative_position_mappings(
        &mut self,
        text: &str,
        kind: cadenza_anchor::DecorativeKind,
    ) {
        if !self.options.track_positions {
            return;
        }
        use unicode_segmentation::UnicodeSegmentation;
        let source_kind = self.current_source_kind();
        for _grapheme in text.graphemes(true) {
            if let Some(line_map) = self.position_map.current_line_mut() {
                line_map.push(CharMapping::new_kinded(
                    self.current_render_col,
                    self.formatting_stack.clone(),
                    None,
                    source_kind,
                    Some(kind),
                ));
            }
            self.current_render_col += 1;
        }
    }

    fn finish_line(&mut self) {
        use unicode_segmentation::UnicodeSegmentation;

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
                // NOTE: wrap path's position_map alignment with the blockquote
                // bar (and per-line wrap-induced indents) is deferred to
                // Step 3b/3d (the wrap_spans rewrite). For the no-wrap path
                // below, the bar is synced via prepend_decoratives.
            } else {
                if let Some(p) = prefix {
                    // Sync position_map: prepend decorative entries for the
                    // blockquote bar so flat-index grapheme lookups stay
                    // aligned with rendered Text columns. (Strong-oracle
                    // round-trip relies on this.)
                    if self.options.track_positions {
                        let prefix_grapheme_count = p.content.graphemes(true).count();
                        if let Some(line) = self.position_map.current_line_mut() {
                            line.prepend_decoratives(
                                prefix_grapheme_count,
                                cadenza_anchor::DecorativeKind::BlockquoteBar,
                                cadenza_anchor::SourceKind::PlainText,
                            );
                        }
                    }
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
    fn wrap_spans(
        &self,
        spans: Vec<RSpan<'static>>,
        prefix: Option<RSpan<'static>>,
    ) -> Vec<Line<'static>> {
        let max_width = self.options.width;
        let prefix_len = prefix.as_ref().map(|p| p.content.width()).unwrap_or(0);
        let effective_width = max_width.saturating_sub(prefix_len);

        let wrapped = wrap_spans_to_width(&spans, effective_width);
        wrapped
            .into_iter()
            .map(|mut line_spans| {
                if let Some(ref p) = prefix {
                    line_spans.insert(0, p.clone());
                }
                Line::from(line_spans)
            })
            .collect()
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
        self.lines.push(Line::from(vec![RSpan::styled(
            hr,
            self.theme.horizontal_rule,
        )]));
    }

    /// Append a fully-rendered table line to `self.lines`, optionally clipping
    /// it to the viewport width with a `▶` marker. Keeps `position_map` in
    /// sync when tracking is enabled so that line indexes past the table
    /// remain valid for selection.
    fn push_table_line(&mut self, line: Line<'static>, truncate: bool) {
        let line = if truncate && self.options.width > 0 {
            truncate_line_to_width(line, self.options.width)
        } else {
            line
        };
        self.lines.push(line);
        if self.options.track_positions {
            self.position_map.start_line();
        }
    }

    fn render_table(&mut self) {
        if self.table_rows.is_empty() {
            return;
        }

        let cols = self.table_columns;
        if cols == 0 {
            self.table_rows.clear();
            self.table_alignments.clear();
            return;
        }

        // Natural column widths (max content width per column).
        let mut natural: Vec<usize> = vec![0; cols];
        for row in &self.table_rows {
            for (i, cell) in row.iter().enumerate() {
                if i < cols {
                    let w: usize = cell.iter().map(|s| s.content.width()).sum();
                    natural[i] = natural[i].max(w);
                }
            }
        }
        for w in &mut natural {
            *w = (*w).max(3);
        }

        let available = self.options.width;
        let mode = self.options.table_mode;
        let min_col = self.options.min_column_width.max(1);
        // Per-row border overhead: leading "│ " (2) + per-column " │ " (3).
        let border_overhead = 2 + 3 * cols;

        let (col_widths, use_truncate): (Vec<usize>, bool) = if available == 0
            || mode == TableMode::Natural
        {
            (natural.clone(), false)
        } else if mode == TableMode::Truncate {
            let overflow = natural.iter().sum::<usize>() + border_overhead > available;
            (natural.clone(), overflow)
        } else {
            // SqueezeWrap: shrink largest column first until we fit or hit min.
            let mut widths = natural.clone();
            loop {
                let total: usize = widths.iter().sum::<usize>() + border_overhead;
                if total <= available {
                    break;
                }
                let (max_idx, &max_w) = match widths.iter().enumerate().max_by_key(|(_, w)| *w) {
                    Some(pair) => pair,
                    None => break,
                };
                if max_w <= min_col {
                    break;
                }
                widths[max_idx] = (max_w - 1).max(min_col);
            }
            let overflow = widths.iter().sum::<usize>() + border_overhead > available;
            (widths, overflow)
        };

        // Take table state so we can iterate without double-borrowing self.
        let rows = std::mem::take(&mut self.table_rows);
        let alignments = std::mem::take(&mut self.table_alignments);
        self.table_columns = 0;

        let border_style = self.theme.table_border;
        let header_style = self.theme.table_header;
        let cell_style = self.theme.table_cell;

        // Top border.
        let top_border: String = col_widths
            .iter()
            .map(|w| "─".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("┬");
        let top_line = Line::from(vec![RSpan::styled(
            format!("┌{}┐", top_border),
            border_style,
        )]);
        self.push_table_line(top_line, use_truncate);

        let row_count = rows.len();
        for (row_idx, row) in rows.iter().enumerate() {
            // Wrap each cell to its column width.
            let wrapped_cells: Vec<Vec<Vec<RSpan<'static>>>> = row
                .iter()
                .enumerate()
                .map(|(col_idx, cell)| {
                    let width = col_widths.get(col_idx).copied().unwrap_or(3);
                    wrap_spans_to_width(cell, width)
                })
                .collect();

            let row_height = wrapped_cells
                .iter()
                .map(|c| c.len().max(1))
                .max()
                .unwrap_or(1);

            let row_style = if row_idx == 0 {
                header_style
            } else {
                cell_style
            };

            for visual_idx in 0..row_height {
                let mut line_spans: Vec<RSpan<'static>> =
                    vec![RSpan::styled("│ ".to_string(), border_style)];

                for col_idx in 0..cols {
                    let width = col_widths.get(col_idx).copied().unwrap_or(3);
                    let align = alignments
                        .get(col_idx)
                        .copied()
                        .unwrap_or(Alignment::Default);

                    let empty: Vec<RSpan<'static>> = Vec::new();
                    let cell_line: &[RSpan<'static>] = wrapped_cells
                        .get(col_idx)
                        .and_then(|lines| lines.get(visual_idx))
                        .map(|v| v.as_slice())
                        .unwrap_or(&empty);

                    let cell_width: usize = cell_line.iter().map(|s| s.content.width()).sum();
                    let pad = width.saturating_sub(cell_width);
                    let (left_pad, right_pad) = match align {
                        Alignment::Center => {
                            let left = pad / 2;
                            (left, pad - left)
                        }
                        Alignment::Right => (pad, 0),
                        _ => (0, pad),
                    };

                    if left_pad > 0 {
                        line_spans.push(RSpan::styled(" ".repeat(left_pad), row_style));
                    }
                    for span in cell_line {
                        line_spans.push(RSpan::styled(
                            span.content.to_string(),
                            span.style.patch(row_style),
                        ));
                    }
                    if right_pad > 0 {
                        line_spans.push(RSpan::styled(" ".repeat(right_pad), row_style));
                    }

                    line_spans.push(RSpan::styled(" │ ".to_string(), border_style));
                }

                self.push_table_line(Line::from(line_spans), use_truncate);
            }

            // Horizontal separator between logical rows (not after the last row).
            if row_idx < row_count - 1 {
                let sep: String = col_widths
                    .iter()
                    .enumerate()
                    .map(|(i, w)| {
                        if row_idx == 0 {
                            let align = alignments.get(i).copied().unwrap_or(Alignment::Default);
                            match align {
                                Alignment::Left => format!(":{}─", "─".repeat(*w)),
                                Alignment::Right => format!("{}─:", "─".repeat(*w)),
                                Alignment::Center => format!(":{}:", "─".repeat(*w)),
                                _ => "─".repeat(*w + 2),
                            }
                        } else {
                            "─".repeat(*w + 2)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("┼");
                let sep_line = Line::from(vec![RSpan::styled(format!("├{}┤", sep), border_style)]);
                self.push_table_line(sep_line, use_truncate);
            }
        }

        // Bottom border.
        let bottom_border: String = col_widths
            .iter()
            .map(|w| "─".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("┴");
        let bottom_line = Line::from(vec![RSpan::styled(
            format!("└{}┘", bottom_border),
            border_style,
        )]);
        self.push_table_line(bottom_line, use_truncate);
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
                        self.current_spans.push(RSpan::styled(prefix.clone(), style));
                        self.push_decorative_position_mappings(
                            &prefix,
                            cadenza_anchor::DecorativeKind::ListBullet,
                        );
                    }
                }
            }

            Block::Heading(HeadingDetail { level }) => {
                self.in_heading = Some(level);
                self.push_style(self.theme.heading_style(level));

                // Render the heading prefix `## ` AND emit matching
                // decorative position mappings so current_render_col stays
                // in sync with the painted cells. Without the matching
                // push_decorative_position_mappings, subsequent heading text
                // would get a wrong render_offset relative to the rendered
                // Text. (Claude Step-3a F13: scope-coverage fix.)
                let prefix = format!("{} ", "#".repeat(level as usize));
                self.current_spans.push(RSpan::styled(
                    prefix.clone(),
                    self.theme.heading_style(level),
                ));
                self.push_decorative_position_mappings(
                    &prefix,
                    cadenza_anchor::DecorativeKind::HeadingMarker,
                );
            }

            Block::Quote => {
                self.in_blockquote = true;
                self.push_style(self.theme.blockquote);
            }

            Block::Code(CodeBlockDetail { lang, .. }) => {
                self.in_code_block = true;

                // Show language label if present. Sync the position_map: emit
                // a matching CodeFenceLabel-decorated line so flat-index
                // grapheme lookups stay aligned with rendered Text rows.
                if !lang.is_empty() {
                    // If the current position_map line has any entries
                    // (real content), finish it first so the label gets its
                    // own clean line. If it's already empty (the common
                    // case: code block is the first content, or there's
                    // already been a finish_line), reuse it directly to
                    // avoid an orphan empty line that throws off the
                    // rendered.text.lines ↔ position_map.lines alignment.
                    let current_line_empty = if self.options.track_positions {
                        self.position_map
                            .current_line_mut()
                            .map_or(true, |l| l.is_empty())
                    } else {
                        true
                    };
                    if !current_line_empty {
                        self.finish_line();
                    }

                    let label = format!("{}:", lang);
                    self.lines.push(Line::from(vec![RSpan::styled(
                        label.clone(),
                        self.theme.code_block_info,
                    )]));

                    if self.options.track_positions {
                        // Fill the current (now-empty) position_map line
                        // with decoratives for the label, then start a
                        // fresh line for the code content that follows.
                        self.push_decorative_position_mappings(
                            &label,
                            cadenza_anchor::DecorativeKind::CodeFenceLabel,
                        );
                        self.current_render_col = 0;
                        self.position_map.start_line();
                    }
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

            Block::TableHeaderCell(TableCellDetail { alignment })
            | Block::TableCell(TableCellDetail { alignment }) => {
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
                    let text: String = self
                        .current_spans
                        .iter()
                        .map(|s| s.content.to_string())
                        .collect();
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
                            let lines = highlighter
                                .highlight(&self.code_block_content, &self.code_block_lang);
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
                self.table_rows
                    .push(std::mem::take(&mut self.current_table_row));
            }

            BlockType::TableHeaderCell | BlockType::TableCell => {
                self.current_table_row
                    .push(std::mem::take(&mut self.current_table_cell));
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
                        self.current_spans.push(RSpan::styled(
                            format!(" ({})", detail.href),
                            self.theme.link_url,
                        ));
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

    fn text(&mut self, text_type: TextType, text: &str, ctx: md4c::TextContext) -> bool {
        // Source-offset propagation policy:
        //   - For `Normal`, `Code`, `Entity`, `Html`, `LatexMath` runs MD4C
        //     delivers from the input buffer, use ctx.source_offset directly.
        //   - For `HardBreak`, `SoftBreak`, `NullChar`: the renderer
        //     synthesizes glyphs (`" "`, `"\u{FFFD}"`, etc.) that are NOT in
        //     the input at any offset. The synthetic text must be mapped as
        //     unmapped (source_offset = None) so per-grapheme spans don't
        //     accidentally point at the prior run's bytes (or the newline
        //     that triggered the break). Caught by codex Step-3a F2.
        self.current_source_offset = match text_type {
            TextType::HardBreak | TextType::SoftBreak | TextType::NullChar => None,
            _ => ctx.source_offset,
        };
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
pub fn render(markdown: &str, theme: &Theme, options: &RenderOptions) -> RenderedMarkdown {
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

/// Render markdown and return both the rendered output AND a
/// `MarkdownSourceMap` scoped to the given block id.
///
/// Used by Cadenza's selection engine: the returned `MarkdownSourceMap`
/// implements `cadenza_anchor::SourceMapping` and is the load-bearing
/// per-block primitive that powers source-mode copy. The block id is
/// supplied by the caller (Cadenza converts from
/// `orchestr8_projection::BlockId` at the boundary).
///
/// Force-enables `track_positions` regardless of the caller's setting —
/// the source map is meaningless without it.
///
/// Force-DISABLES `syntax_highlighting` until Step 3b lands the
/// syntect-side parallel byte-offset side-channel (per the plan §III.3
/// sub-gate 3b). With highlighting on, code-block text is buffered in
/// `push_text` and the highlighted lines are appended in `leave_block`
/// without going through the position-tracking path — that produces a
/// partial map whose render-column accounting is wrong on syntax-
/// highlighted code blocks. Until 3b, callers who want a usable source
/// map for code-heavy content must accept unhighlighted code (the
/// rendered text is still correct; only the per-token colors are
/// missing). Caught by codex Step-3a F3.
pub fn render_with_block(
    markdown: &str,
    theme: &Theme,
    options: &RenderOptions,
    block_id: cadenza_anchor::BlockId,
) -> (RenderedMarkdown, crate::source_map::MarkdownSourceMap) {
    let mut opts = options.clone();
    opts.track_positions = true;
    opts.syntax_highlighting = false;
    let rendered = render(markdown, theme, &opts);
    let pm = rendered
        .position_map
        .clone()
        .expect("track_positions was forced on; position_map must be Some");
    let source: std::sync::Arc<str> = std::sync::Arc::from(markdown);
    let source_map = crate::source_map::MarkdownSourceMap::new(block_id, source, pm);
    (rendered, source_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_rendering() {
        let result = render(
            "Hello **world**",
            &Theme::default(),
            &RenderOptions::default(),
        );
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
        let result = render(
            "- item 1\n- item 2",
            &Theme::default(),
            &RenderOptions::default(),
        );
        assert!(result.text.lines.len() >= 2);
    }

    #[test]
    fn test_code_block() {
        let result = render(
            "```rust\nfn main() {}\n```",
            &Theme::default(),
            &RenderOptions::default(),
        );
        assert!(!result.text.lines.is_empty());
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

    fn render_lines_as_strings(result: &RenderedMarkdown) -> Vec<String> {
        result
            .text
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn test_table_squeezes_to_width() {
        let md = "| Name | Description | Status |\n|------|-------------|--------|\n| alpha | the quick brown fox jumps over the lazy dog | ok |\n| beta | another reasonably long descriptive entry | done |";
        let opts = RenderOptions::github().with_width(40);
        let result = render(md, &Theme::default(), &opts);

        for line in &result.text.lines {
            let w: usize = line.spans.iter().map(|s| s.content.width()).sum();
            assert!(
                w <= 40,
                "table line exceeds width 40 (got {}): {:?}",
                w,
                line.spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
            );
        }
    }

    #[test]
    fn test_table_wraps_cell_contents() {
        // Narrow viewport forces the Description column to wrap.
        let md = "| Id | Description |\n|----|-------------|\n| 1 | the quick brown fox jumps over the lazy dog |\n| 2 | short |";
        let opts = RenderOptions::github().with_width(30);
        let result = render(md, &Theme::default(), &opts);

        let lines = render_lines_as_strings(&result);

        // Count separators (├): should be exactly one per gap between logical rows.
        // Rows: header, data1, data2 => 2 gaps => 2 separators.
        let sep_count = lines.iter().filter(|l| l.contains("├")).count();
        assert_eq!(
            sep_count,
            2,
            "expected 2 horizontal separators, got {}. Lines:\n{}",
            sep_count,
            lines.join("\n")
        );

        // The data row for id=1 should span multiple visual lines: find the row
        // containing "quick" and verify the continuation line stays inside
        // borders and does not start a new separator.
        let quick_idx = lines
            .iter()
            .position(|l| l.contains("quick"))
            .expect("wrapped row should contain 'quick'");
        // The line after should either continue the same row (starts with "│")
        // or be the separator. It must not be the bottom border.
        let next = &lines[quick_idx + 1];
        assert!(
            next.starts_with("│") || next.starts_with("├"),
            "line after wrapped cell should continue row or start separator, got: {:?}",
            next
        );
    }

    #[test]
    fn test_table_min_column_fallback() {
        // Extremely narrow viewport for 4 columns; should not panic.
        let md = "| A | B | C | D |\n|---|---|---|---|\n| foo | bar | baz | qux |";
        let opts = RenderOptions::github()
            .with_width(10)
            .with_min_column_width(2);
        let result = render(md, &Theme::default(), &opts);
        // At min widths, total still exceeds 10, so TableMode::SqueezeWrap
        // falls through to truncation: every line must be clipped to width 10.
        for line in &result.text.lines {
            let w: usize = line.spans.iter().map(|s| s.content.width()).sum();
            assert!(w <= 10, "truncated table line exceeds 10: width={}", w);
        }
        // At least one line should carry the truncation marker.
        let has_marker = result
            .text
            .lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains('▶')));
        assert!(has_marker, "expected ▶ truncation indicator somewhere");
    }

    #[test]
    fn test_table_alignment_preserved_after_shrink() {
        let md = "| Left | Center | Right |\n|:-----|:------:|------:|\n| aaaaaaaaaa | bbbbbbbbbb | cccccccccc |";
        let opts = RenderOptions::github().with_width(30);
        let result = render(md, &Theme::default(), &opts);
        let lines = render_lines_as_strings(&result);

        // Header separator (first separator after the top border) must retain
        // alignment markers.
        let sep = lines
            .iter()
            .find(|l| l.contains("├"))
            .expect("expected header separator");
        // Left-aligned: `:---`, Center: `:---:`, Right: `---:`
        assert!(
            sep.contains(":"),
            "header separator should retain alignment markers, got: {}",
            sep
        );
    }

    #[test]
    fn test_table_natural_when_width_zero() {
        // width=0 means "unknown viewport"; table should render at natural widths.
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let opts = RenderOptions::github().with_width(0);
        let result = render(md, &Theme::default(), &opts);
        assert!(result.text.lines.len() >= 3);
    }

    #[test]
    fn test_table_position_map_matches_line_count() {
        let md = "| Id | Description |\n|----|-------------|\n| 1 | long text that will wrap across multiple lines in a narrow view |\n| 2 | ok |";
        let opts = RenderOptions::github()
            .with_width(24)
            .with_position_tracking(true);
        let result = render(md, &Theme::default(), &opts);

        let position_map = result.position_map.as_ref().expect("position_map present");
        // Invariant (see render()): position_map has one trailing empty line
        // beyond the finalized lines.
        assert!(
            position_map.line_count() >= result.line_count,
            "position_map line_count ({}) should be >= rendered line_count ({})",
            position_map.line_count(),
            result.line_count
        );
    }

    #[test]
    fn test_table_row_separators() {
        // Test that tables render with separators between all rows (not just header)
        let markdown = "| Name | Age | Status |\n|------|-----|--------|\n| Alice | 30 | Active |\n| Bob | 25 | Inactive |";
        let result = render(markdown, &Theme::default(), &RenderOptions::github());

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
        assert_eq!(
            separator_count, 2,
            "Should have 2 separators (after header and after first data row), got {}",
            separator_count
        );

        // Verify both separators use ┼ for column junctions
        for line in &rendered_text {
            if line.contains("├") {
                assert!(
                    line.contains("┼"),
                    "Separator line should contain column junctions (┼): {}",
                    line
                );
            }
        }
    }
}

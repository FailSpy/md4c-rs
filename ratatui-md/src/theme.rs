//! Theme configuration for markdown rendering.
//!
//! Provides customizable styling for all markdown elements.

use ratatui::style::{Color, Modifier, Style};

/// Theme configuration for rendering markdown.
///
/// Each field controls the style of a specific markdown element type.
/// Use `Theme::default()` for sensible defaults, or create a custom theme.
///
/// # Example
///
/// ```
/// use ratatui_md::Theme;
/// use ratatui::style::{Color, Modifier, Style};
///
/// let mut theme = Theme::default();
/// theme.heading1 = Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD);
/// ```
#[derive(Debug, Clone)]
pub struct Theme {
    /// Normal text style
    pub text: Style,

    /// Emphasis (italic) style - typically *text* or _text_
    pub emphasis: Style,

    /// Strong emphasis (bold) style - typically **text** or __text__
    pub strong: Style,

    /// Strikethrough style - ~~text~~
    pub strikethrough: Style,

    /// Underline style (MD4C extension)
    pub underline: Style,

    /// Inline code style - `code`
    pub code_inline: Style,

    /// Code block style (the code content itself)
    pub code_block: Style,

    /// Code block info/language label style
    pub code_block_info: Style,

    /// Link text style
    pub link: Style,

    /// Link URL style (when shown)
    pub link_url: Style,

    /// Image alt text style
    pub image: Style,

    /// H1 heading style
    pub heading1: Style,

    /// H2 heading style
    pub heading2: Style,

    /// H3 heading style
    pub heading3: Style,

    /// H4 heading style
    pub heading4: Style,

    /// H5 heading style
    pub heading5: Style,

    /// H6 heading style
    pub heading6: Style,

    /// Blockquote text style
    pub blockquote: Style,

    /// Blockquote marker/border style
    pub blockquote_marker: Style,

    /// Horizontal rule style
    pub horizontal_rule: Style,

    /// Unordered list bullet marker style
    pub list_bullet: Style,

    /// Ordered list number marker style
    pub list_number: Style,

    /// Task list unchecked marker [ ] style
    pub task_unchecked: Style,

    /// Task list checked marker [x] style
    pub task_checked: Style,

    /// Table header cell style
    pub table_header: Style,

    /// Table cell style
    pub table_cell: Style,

    /// Table border style
    pub table_border: Style,

    /// HTML entity style (rendered literally if not decoded)
    pub html_entity: Style,

    /// Raw HTML style
    pub raw_html: Style,

    /// LaTeX math style
    pub latex_math: Style,

    /// Wiki link style
    pub wiki_link: Style,

    // === Rendering options ===
    /// Character used for unordered list bullets
    pub bullet_char: char,

    /// Character used for horizontal rules (repeated)
    pub hr_char: char,

    /// String used for blockquote markers
    pub blockquote_prefix: &'static str,

    /// Show link URLs inline after link text
    pub show_link_urls: bool,

    /// Indent size for nested lists (in spaces)
    pub list_indent: usize,

    /// Character for unchecked task list items
    pub task_unchecked_char: char,

    /// Character for checked task list items
    pub task_checked_char: char,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            text: Style::default(),
            emphasis: Style::default().add_modifier(Modifier::ITALIC),
            strong: Style::default().add_modifier(Modifier::BOLD),
            strikethrough: Style::default().add_modifier(Modifier::CROSSED_OUT),
            underline: Style::default().add_modifier(Modifier::UNDERLINED),
            code_inline: Style::default().fg(Color::Yellow),
            code_block: Style::default().fg(Color::White),
            code_block_info: Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            link: Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
            link_url: Style::default().fg(Color::DarkGray),
            image: Style::default().fg(Color::Magenta),
            heading1: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            heading2: Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            heading3: Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            heading4: Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            heading5: Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            heading6: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            blockquote: Style::default().fg(Color::Gray),
            blockquote_marker: Style::default().fg(Color::DarkGray),
            horizontal_rule: Style::default().fg(Color::DarkGray),
            list_bullet: Style::default().fg(Color::Cyan),
            list_number: Style::default().fg(Color::Cyan),
            task_unchecked: Style::default().fg(Color::DarkGray),
            task_checked: Style::default().fg(Color::Green),
            table_header: Style::default().add_modifier(Modifier::BOLD),
            table_cell: Style::default(),
            table_border: Style::default().fg(Color::DarkGray),
            html_entity: Style::default().fg(Color::Yellow),
            raw_html: Style::default().fg(Color::DarkGray),
            latex_math: Style::default().fg(Color::Magenta),
            wiki_link: Style::default().fg(Color::Blue).add_modifier(Modifier::UNDERLINED),

            bullet_char: '•',
            hr_char: '─',
            blockquote_prefix: "│ ",
            show_link_urls: false,
            list_indent: 2,
            task_unchecked_char: '☐',
            task_checked_char: '☑',
        }
    }
}

impl Theme {
    /// Create a new theme with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a minimal theme with no colors (for plain terminals).
    pub fn plain() -> Self {
        Self {
            text: Style::default(),
            emphasis: Style::default().add_modifier(Modifier::ITALIC),
            strong: Style::default().add_modifier(Modifier::BOLD),
            strikethrough: Style::default().add_modifier(Modifier::CROSSED_OUT),
            underline: Style::default().add_modifier(Modifier::UNDERLINED),
            code_inline: Style::default(),
            code_block: Style::default(),
            code_block_info: Style::default().add_modifier(Modifier::DIM),
            link: Style::default().add_modifier(Modifier::UNDERLINED),
            link_url: Style::default().add_modifier(Modifier::DIM),
            image: Style::default(),
            heading1: Style::default().add_modifier(Modifier::BOLD),
            heading2: Style::default().add_modifier(Modifier::BOLD),
            heading3: Style::default().add_modifier(Modifier::BOLD),
            heading4: Style::default().add_modifier(Modifier::BOLD),
            heading5: Style::default().add_modifier(Modifier::BOLD),
            heading6: Style::default().add_modifier(Modifier::BOLD),
            blockquote: Style::default().add_modifier(Modifier::DIM),
            blockquote_marker: Style::default(),
            horizontal_rule: Style::default(),
            list_bullet: Style::default(),
            list_number: Style::default(),
            task_unchecked: Style::default(),
            task_checked: Style::default(),
            table_header: Style::default().add_modifier(Modifier::BOLD),
            table_cell: Style::default(),
            table_border: Style::default(),
            html_entity: Style::default(),
            raw_html: Style::default().add_modifier(Modifier::DIM),
            latex_math: Style::default(),
            wiki_link: Style::default().add_modifier(Modifier::UNDERLINED),

            bullet_char: '*',
            hr_char: '-',
            blockquote_prefix: "> ",
            show_link_urls: true,
            list_indent: 2,
            task_unchecked_char: ' ',
            task_checked_char: 'x',
        }
    }

    /// Create a dark theme optimized for dark terminal backgrounds.
    pub fn dark() -> Self {
        Self {
            text: Style::default().fg(Color::White),
            emphasis: Style::default().fg(Color::White).add_modifier(Modifier::ITALIC),
            strong: Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            strikethrough: Style::default().fg(Color::Gray).add_modifier(Modifier::CROSSED_OUT),
            underline: Style::default().fg(Color::White).add_modifier(Modifier::UNDERLINED),
            code_inline: Style::default().fg(Color::LightYellow).bg(Color::DarkGray),
            code_block: Style::default().fg(Color::LightYellow),
            code_block_info: Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
            link: Style::default().fg(Color::LightCyan).add_modifier(Modifier::UNDERLINED),
            link_url: Style::default().fg(Color::Gray),
            image: Style::default().fg(Color::LightMagenta),
            heading1: Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD),
            heading2: Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD),
            heading3: Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
            heading4: Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD),
            heading5: Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD),
            heading6: Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD),
            blockquote: Style::default().fg(Color::Gray),
            blockquote_marker: Style::default().fg(Color::DarkGray),
            horizontal_rule: Style::default().fg(Color::DarkGray),
            list_bullet: Style::default().fg(Color::LightCyan),
            list_number: Style::default().fg(Color::LightCyan),
            task_unchecked: Style::default().fg(Color::Gray),
            task_checked: Style::default().fg(Color::LightGreen),
            table_header: Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            table_cell: Style::default().fg(Color::White),
            table_border: Style::default().fg(Color::DarkGray),
            html_entity: Style::default().fg(Color::LightYellow),
            raw_html: Style::default().fg(Color::Gray),
            latex_math: Style::default().fg(Color::LightMagenta),
            wiki_link: Style::default().fg(Color::LightBlue).add_modifier(Modifier::UNDERLINED),
            ..Self::default()
        }
    }

    /// Create a light theme optimized for light terminal backgrounds.
    pub fn light() -> Self {
        Self {
            text: Style::default().fg(Color::Black),
            emphasis: Style::default().fg(Color::Black).add_modifier(Modifier::ITALIC),
            strong: Style::default().fg(Color::Black).add_modifier(Modifier::BOLD),
            strikethrough: Style::default().fg(Color::DarkGray).add_modifier(Modifier::CROSSED_OUT),
            underline: Style::default().fg(Color::Black).add_modifier(Modifier::UNDERLINED),
            code_inline: Style::default().fg(Color::Red),
            code_block: Style::default().fg(Color::Black),
            code_block_info: Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            link: Style::default().fg(Color::Blue).add_modifier(Modifier::UNDERLINED),
            link_url: Style::default().fg(Color::DarkGray),
            image: Style::default().fg(Color::Magenta),
            heading1: Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            heading2: Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            heading3: Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            heading4: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            heading5: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            heading6: Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            blockquote: Style::default().fg(Color::DarkGray),
            blockquote_marker: Style::default().fg(Color::Gray),
            horizontal_rule: Style::default().fg(Color::Gray),
            list_bullet: Style::default().fg(Color::Blue),
            list_number: Style::default().fg(Color::Blue),
            task_unchecked: Style::default().fg(Color::DarkGray),
            task_checked: Style::default().fg(Color::Green),
            table_header: Style::default().fg(Color::Black).add_modifier(Modifier::BOLD),
            table_cell: Style::default().fg(Color::Black),
            table_border: Style::default().fg(Color::Gray),
            html_entity: Style::default().fg(Color::Red),
            raw_html: Style::default().fg(Color::DarkGray),
            latex_math: Style::default().fg(Color::Magenta),
            wiki_link: Style::default().fg(Color::Blue).add_modifier(Modifier::UNDERLINED),
            ..Self::default()
        }
    }

    /// Get the style for a heading by level (1-6).
    pub fn heading_style(&self, level: u8) -> Style {
        match level {
            1 => self.heading1,
            2 => self.heading2,
            3 => self.heading3,
            4 => self.heading4,
            5 => self.heading5,
            _ => self.heading6,
        }
    }

    /// Builder method to set link URL display.
    pub fn with_link_urls(mut self, show: bool) -> Self {
        self.show_link_urls = show;
        self
    }

    /// Builder method to set bullet character.
    pub fn with_bullet(mut self, c: char) -> Self {
        self.bullet_char = c;
        self
    }

    /// Builder method to set list indentation.
    pub fn with_list_indent(mut self, indent: usize) -> Self {
        self.list_indent = indent;
        self
    }
}

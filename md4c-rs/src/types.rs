//! Rust-friendly types for MD4C parsing.

use crate::sys;
use std::os::raw::c_char;

/// Block element types in Markdown
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BlockType {
    /// Document root
    Document,
    /// Block quote
    Quote,
    /// Unordered list
    UnorderedList,
    /// Ordered list
    OrderedList,
    /// List item
    ListItem,
    /// Horizontal rule
    HorizontalRule,
    /// Heading (h1-h6)
    Heading,
    /// Code block (fenced or indented)
    Code,
    /// Raw HTML block
    Html,
    /// Paragraph
    Paragraph,
    /// Table (extension)
    Table,
    /// Table header section
    TableHead,
    /// Table body section
    TableBody,
    /// Table row
    TableRow,
    /// Table header cell
    TableHeaderCell,
    /// Table data cell
    TableCell,
}

impl BlockType {
    pub(crate) fn from_raw(raw: sys::MD_BLOCKTYPE) -> Option<Self> {
        match raw {
            sys::MD_BLOCK_DOC => Some(BlockType::Document),
            sys::MD_BLOCK_QUOTE => Some(BlockType::Quote),
            sys::MD_BLOCK_UL => Some(BlockType::UnorderedList),
            sys::MD_BLOCK_OL => Some(BlockType::OrderedList),
            sys::MD_BLOCK_LI => Some(BlockType::ListItem),
            sys::MD_BLOCK_HR => Some(BlockType::HorizontalRule),
            sys::MD_BLOCK_H => Some(BlockType::Heading),
            sys::MD_BLOCK_CODE => Some(BlockType::Code),
            sys::MD_BLOCK_HTML => Some(BlockType::Html),
            sys::MD_BLOCK_P => Some(BlockType::Paragraph),
            sys::MD_BLOCK_TABLE => Some(BlockType::Table),
            sys::MD_BLOCK_THEAD => Some(BlockType::TableHead),
            sys::MD_BLOCK_TBODY => Some(BlockType::TableBody),
            sys::MD_BLOCK_TR => Some(BlockType::TableRow),
            sys::MD_BLOCK_TH => Some(BlockType::TableHeaderCell),
            sys::MD_BLOCK_TD => Some(BlockType::TableCell),
            _ => None,
        }
    }
}

/// Inline span types in Markdown
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SpanType {
    /// Emphasis (italic)
    Emphasis,
    /// Strong emphasis (bold)
    Strong,
    /// Link
    Link,
    /// Image
    Image,
    /// Inline code
    Code,
    /// Strikethrough (extension)
    Strikethrough,
    /// Inline LaTeX math (extension)
    LatexMath,
    /// Display LaTeX math (extension)
    LatexMathDisplay,
    /// Wiki link (extension)
    WikiLink,
    /// Underline (extension)
    Underline,
    /// Spoiler (extension)
    Spoiler,
}

impl SpanType {
    pub(crate) fn from_raw(raw: sys::MD_SPANTYPE) -> Option<Self> {
        match raw {
            sys::MD_SPAN_EM => Some(SpanType::Emphasis),
            sys::MD_SPAN_STRONG => Some(SpanType::Strong),
            sys::MD_SPAN_A => Some(SpanType::Link),
            sys::MD_SPAN_IMG => Some(SpanType::Image),
            sys::MD_SPAN_CODE => Some(SpanType::Code),
            sys::MD_SPAN_DEL => Some(SpanType::Strikethrough),
            sys::MD_SPAN_LATEXMATH => Some(SpanType::LatexMath),
            sys::MD_SPAN_LATEXMATH_DISPLAY => Some(SpanType::LatexMathDisplay),
            sys::MD_SPAN_WIKILINK => Some(SpanType::WikiLink),
            sys::MD_SPAN_U => Some(SpanType::Underline),
            sys::MD_SPAN_SPOILER => Some(SpanType::Spoiler),
            _ => None,
        }
    }
}

/// Text content types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextType {
    /// Normal text
    Normal,
    /// Null character replacement (U+FFFD)
    NullChar,
    /// Hard line break
    HardBreak,
    /// Soft line break (newline in source)
    SoftBreak,
    /// HTML entity (&nbsp;, &#123;, etc.)
    Entity,
    /// Code content
    Code,
    /// Raw HTML content
    Html,
    /// LaTeX math content
    LatexMath,
}

impl TextType {
    pub(crate) fn from_raw(raw: sys::MD_TEXTTYPE) -> Option<Self> {
        match raw {
            sys::MD_TEXT_NORMAL => Some(TextType::Normal),
            sys::MD_TEXT_NULLCHAR => Some(TextType::NullChar),
            sys::MD_TEXT_BR => Some(TextType::HardBreak),
            sys::MD_TEXT_SOFTBR => Some(TextType::SoftBreak),
            sys::MD_TEXT_ENTITY => Some(TextType::Entity),
            sys::MD_TEXT_CODE => Some(TextType::Code),
            sys::MD_TEXT_HTML => Some(TextType::Html),
            sys::MD_TEXT_LATEXMATH => Some(TextType::LatexMath),
            _ => None,
        }
    }
}

/// Table cell alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Alignment {
    /// Default alignment (typically left)
    #[default]
    Default,
    /// Left aligned
    Left,
    /// Center aligned
    Center,
    /// Right aligned
    Right,
}

impl Alignment {
    pub(crate) fn from_raw(raw: sys::MD_ALIGN) -> Self {
        match raw {
            sys::MD_ALIGN_LEFT => Alignment::Left,
            sys::MD_ALIGN_CENTER => Alignment::Center,
            sys::MD_ALIGN_RIGHT => Alignment::Right,
            _ => Alignment::Default,
        }
    }
}

/// List marker character for unordered lists
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListMark {
    /// Dash `-`
    Dash,
    /// Plus `+`
    Plus,
    /// Asterisk `*`
    Asterisk,
}

impl ListMark {
    pub(crate) fn from_raw(raw: c_char) -> Self {
        match raw as u8 as char {
            '-' => ListMark::Dash,
            '+' => ListMark::Plus,
            _ => ListMark::Asterisk,
        }
    }
}

/// Ordered list delimiter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrderedListDelimiter {
    /// Period `.`
    Period,
    /// Parenthesis `)`
    Parenthesis,
}

impl OrderedListDelimiter {
    pub(crate) fn from_raw(raw: c_char) -> Self {
        match raw as u8 as char {
            ')' => OrderedListDelimiter::Parenthesis,
            _ => OrderedListDelimiter::Period,
        }
    }
}

/// Fence character for code blocks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FenceChar {
    /// Backtick `` ` ``
    Backtick,
    /// Tilde `~`
    Tilde,
    /// Indented code block (no fence character)
    Indented,
}

impl FenceChar {
    pub(crate) fn from_raw(raw: c_char) -> Self {
        match raw as u8 as char {
            '`' => FenceChar::Backtick,
            '~' => FenceChar::Tilde,
            _ => FenceChar::Indented,
        }
    }
}

/// Task list item state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskState {
    /// Not a task list item
    NotTask,
    /// Unchecked `[ ]`
    Unchecked,
    /// Checked `[x]` or `[X]`
    Checked,
}

/// Detail information for unordered lists
#[derive(Debug, Clone)]
pub struct UnorderedListDetail {
    /// Whether this is a tight list (no blank lines between items)
    pub is_tight: bool,
    /// The marker character used
    pub mark: ListMark,
}

/// Detail information for ordered lists
#[derive(Debug, Clone)]
pub struct OrderedListDetail {
    /// Starting number
    pub start: u32,
    /// Whether this is a tight list
    pub is_tight: bool,
    /// The delimiter used (`.` or `)`)
    pub delimiter: OrderedListDelimiter,
}

/// Detail information for list items
#[derive(Debug, Clone)]
pub struct ListItemDetail {
    /// Task state (if task list extension is enabled)
    pub task_state: TaskState,
}

/// Detail information for headings
#[derive(Debug, Clone)]
pub struct HeadingDetail {
    /// Heading level (1-6)
    pub level: u8,
}

/// Detail information for code blocks
#[derive(Debug, Clone)]
pub struct CodeBlockDetail {
    /// Info string (everything after the opening fence)
    pub info: String,
    /// Language identifier (first word of info string)
    pub lang: String,
    /// Fence character used
    pub fence_char: FenceChar,
}

/// Detail information for tables
#[derive(Debug, Clone)]
pub struct TableDetail {
    /// Number of columns
    pub column_count: u32,
    /// Number of header rows (typically 1)
    pub head_row_count: u32,
    /// Number of body rows
    pub body_row_count: u32,
}

/// Detail information for table cells
#[derive(Debug, Clone)]
pub struct TableCellDetail {
    /// Cell alignment
    pub alignment: Alignment,
}

/// Detail information for links
#[derive(Debug, Clone)]
pub struct LinkDetail {
    /// Link URL
    pub href: String,
    /// Link title (optional)
    pub title: String,
    /// Whether this is an autolink
    pub is_autolink: bool,
}

/// Detail information for images
#[derive(Debug, Clone)]
pub struct ImageDetail {
    /// Image source URL
    pub src: String,
    /// Image title (optional)
    pub title: String,
}

/// Detail information for wiki links
#[derive(Debug, Clone)]
pub struct WikiLinkDetail {
    /// Wiki link target
    pub target: String,
}

/// Block element with its associated detail information
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Block {
    /// Document root
    Document,
    /// Block quote
    Quote,
    /// Unordered list
    UnorderedList(UnorderedListDetail),
    /// Ordered list
    OrderedList(OrderedListDetail),
    /// List item
    ListItem(ListItemDetail),
    /// Horizontal rule
    HorizontalRule,
    /// Heading
    Heading(HeadingDetail),
    /// Code block
    Code(CodeBlockDetail),
    /// Raw HTML block
    Html,
    /// Paragraph
    Paragraph,
    /// Table
    Table(TableDetail),
    /// Table header section
    TableHead,
    /// Table body section
    TableBody,
    /// Table row
    TableRow,
    /// Table header cell
    TableHeaderCell(TableCellDetail),
    /// Table data cell
    TableCell(TableCellDetail),
}

/// Inline span with its associated detail information
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Span {
    /// Emphasis (italic)
    Emphasis,
    /// Strong emphasis (bold)
    Strong,
    /// Link
    Link(LinkDetail),
    /// Image
    Image(ImageDetail),
    /// Inline code
    Code,
    /// Strikethrough
    Strikethrough,
    /// Inline LaTeX math
    LatexMath,
    /// Display LaTeX math
    LatexMathDisplay,
    /// Wiki link
    WikiLink(WikiLinkDetail),
    /// Underline
    Underline,
    /// Spoiler
    Spoiler,
}

/// Helper to extract string from MD_ATTRIBUTE
pub(crate) unsafe fn attribute_to_string(attr: &sys::MD_ATTRIBUTE) -> String {
    if attr.text.is_null() || attr.size == 0 {
        return String::new();
    }
    let slice = std::slice::from_raw_parts(attr.text as *const u8, attr.size as usize);
    String::from_utf8_lossy(slice).into_owned()
}

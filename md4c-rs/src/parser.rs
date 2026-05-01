//! Safe Rust wrapper for the MD4C parser.

use crate::sys;
use crate::types::*;
use std::os::raw::{c_int, c_void};

/// Parser configuration flags
#[derive(Debug, Clone, Copy, Default)]
pub struct ParserFlags {
    flags: u32,
}

impl ParserFlags {
    /// Create empty flags (CommonMark dialect)
    pub const fn new() -> Self {
        ParserFlags { flags: 0 }
    }

    /// CommonMark dialect (no extensions)
    pub const fn commonmark() -> Self {
        ParserFlags {
            flags: sys::MD_DIALECT_COMMONMARK,
        }
    }

    /// GitHub-flavored Markdown dialect
    pub const fn github() -> Self {
        ParserFlags {
            flags: sys::MD_DIALECT_GITHUB,
        }
    }

    /// Collapse non-trivial whitespace
    pub const fn collapse_whitespace(mut self) -> Self {
        self.flags |= sys::MD_FLAG_COLLAPSEWHITESPACE;
        self
    }

    /// Allow ATX headers without space after `#`
    pub const fn permissive_atx_headers(mut self) -> Self {
        self.flags |= sys::MD_FLAG_PERMISSIVEATXHEADERS;
        self
    }

    /// Recognize URLs as autolinks
    pub const fn permissive_url_autolinks(mut self) -> Self {
        self.flags |= sys::MD_FLAG_PERMISSIVEURLAUTOLINKS;
        self
    }

    /// Recognize email addresses as autolinks
    pub const fn permissive_email_autolinks(mut self) -> Self {
        self.flags |= sys::MD_FLAG_PERMISSIVEEMAILAUTOLINKS;
        self
    }

    /// Recognize www. URLs as autolinks
    pub const fn permissive_www_autolinks(mut self) -> Self {
        self.flags |= sys::MD_FLAG_PERMISSIVEWWWAUTOLINKS;
        self
    }

    /// Enable all permissive autolinks
    pub const fn permissive_autolinks(mut self) -> Self {
        self.flags |= sys::MD_FLAG_PERMISSIVEAUTOLINKS;
        self
    }

    /// Disable indented code blocks
    pub const fn no_indented_code_blocks(mut self) -> Self {
        self.flags |= sys::MD_FLAG_NOINDENTEDCODEBLOCKS;
        self
    }

    /// Disable HTML blocks
    pub const fn no_html_blocks(mut self) -> Self {
        self.flags |= sys::MD_FLAG_NOHTMLBLOCKS;
        self
    }

    /// Disable HTML spans
    pub const fn no_html_spans(mut self) -> Self {
        self.flags |= sys::MD_FLAG_NOHTMLSPANS;
        self
    }

    /// Disable all HTML
    pub const fn no_html(mut self) -> Self {
        self.flags |= sys::MD_FLAG_NOHTML;
        self
    }

    /// Enable tables extension
    pub const fn tables(mut self) -> Self {
        self.flags |= sys::MD_FLAG_TABLES;
        self
    }

    /// Enable strikethrough extension
    pub const fn strikethrough(mut self) -> Self {
        self.flags |= sys::MD_FLAG_STRIKETHROUGH;
        self
    }

    /// Enable task lists extension
    pub const fn task_lists(mut self) -> Self {
        self.flags |= sys::MD_FLAG_TASKLISTS;
        self
    }

    /// Enable LaTeX math spans extension
    pub const fn latex_math_spans(mut self) -> Self {
        self.flags |= sys::MD_FLAG_LATEXMATHSPANS;
        self
    }

    /// Enable wiki links extension
    pub const fn wiki_links(mut self) -> Self {
        self.flags |= sys::MD_FLAG_WIKILINKS;
        self
    }

    /// Enable underline extension
    pub const fn underline(mut self) -> Self {
        self.flags |= sys::MD_FLAG_UNDERLINE;
        self
    }

    /// Treat all soft breaks as hard breaks
    pub const fn hard_soft_breaks(mut self) -> Self {
        self.flags |= sys::MD_FLAG_HARD_SOFT_BREAKS;
        self
    }

    /// Enable spoiler spans extension (`||hidden||`)
    pub const fn spoilers(mut self) -> Self {
        self.flags |= sys::MD_FLAG_SPOILERS;
        self
    }

    /// Get the raw flags value
    pub const fn raw(self) -> u32 {
        self.flags
    }
}

/// Events emitted during parsing
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Event {
    /// Entering a block element
    EnterBlock(Block),
    /// Leaving a block element
    LeaveBlock(BlockType),
    /// Entering an inline span
    EnterSpan(Span),
    /// Leaving an inline span
    LeaveSpan(SpanType),
    /// Text content
    Text(TextType, String),
}

/// Result type for parser operations
pub type ParseResult<T> = Result<T, ParseError>;

/// Error type for parsing operations
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ParseError {
    /// Parser encountered a runtime error
    RuntimeError,
    /// Callback returned an error
    CallbackError(i32),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::RuntimeError => write!(f, "MD4C runtime error"),
            ParseError::CallbackError(code) => write!(f, "Callback error: {}", code),
        }
    }
}

impl std::error::Error for ParseError {}

/// Trait for handling parser events
pub trait ParserHandler {
    /// Called when entering a block element
    fn enter_block(&mut self, block: Block) -> bool {
        let _ = block;
        true
    }

    /// Called when leaving a block element
    fn leave_block(&mut self, block_type: BlockType) -> bool {
        let _ = block_type;
        true
    }

    /// Called when entering an inline span
    fn enter_span(&mut self, span: Span) -> bool {
        let _ = span;
        true
    }

    /// Called when leaving an inline span
    fn leave_span(&mut self, span_type: SpanType) -> bool {
        let _ = span_type;
        true
    }

    /// Called with text content
    fn text(&mut self, text_type: TextType, text: &str) -> bool {
        let _ = (text_type, text);
        true
    }
}

/// Parse markdown text with a custom handler
pub fn parse<H: ParserHandler>(
    input: &str,
    flags: ParserFlags,
    handler: &mut H,
) -> ParseResult<()> {
    struct Context<'a, H: ParserHandler> {
        handler: &'a mut H,
        error: Option<i32>,
    }

    unsafe extern "C" fn enter_block_cb<H: ParserHandler>(
        block_type: sys::MD_BLOCKTYPE,
        detail: *mut c_void,
        userdata: *mut c_void,
    ) -> c_int {
        let ctx = &mut *(userdata as *mut Context<H>);
        let block = parse_block(block_type, detail);
        if ctx.handler.enter_block(block) {
            0
        } else {
            ctx.error = Some(1);
            1
        }
    }

    unsafe extern "C" fn leave_block_cb<H: ParserHandler>(
        block_type: sys::MD_BLOCKTYPE,
        _detail: *mut c_void,
        userdata: *mut c_void,
    ) -> c_int {
        let ctx = &mut *(userdata as *mut Context<H>);
        let bt = BlockType::from_raw(block_type).unwrap_or(BlockType::Document);
        if ctx.handler.leave_block(bt) {
            0
        } else {
            ctx.error = Some(1);
            1
        }
    }

    unsafe extern "C" fn enter_span_cb<H: ParserHandler>(
        span_type: sys::MD_SPANTYPE,
        detail: *mut c_void,
        userdata: *mut c_void,
    ) -> c_int {
        let ctx = &mut *(userdata as *mut Context<H>);
        let span = parse_span(span_type, detail);
        if ctx.handler.enter_span(span) {
            0
        } else {
            ctx.error = Some(1);
            1
        }
    }

    unsafe extern "C" fn leave_span_cb<H: ParserHandler>(
        span_type: sys::MD_SPANTYPE,
        _detail: *mut c_void,
        userdata: *mut c_void,
    ) -> c_int {
        let ctx = &mut *(userdata as *mut Context<H>);
        let st = SpanType::from_raw(span_type).unwrap_or(SpanType::Emphasis);
        if ctx.handler.leave_span(st) {
            0
        } else {
            ctx.error = Some(1);
            1
        }
    }

    unsafe extern "C" fn text_cb<H: ParserHandler>(
        text_type: sys::MD_TEXTTYPE,
        text: *const sys::MD_CHAR,
        size: sys::MD_SIZE,
        userdata: *mut c_void,
    ) -> c_int {
        let ctx = &mut *(userdata as *mut Context<H>);
        let tt = TextType::from_raw(text_type).unwrap_or(TextType::Normal);
        let slice = std::slice::from_raw_parts(text as *const u8, size as usize);
        let text_str = std::str::from_utf8_unchecked(slice);
        if ctx.handler.text(tt, text_str) {
            0
        } else {
            ctx.error = Some(1);
            1
        }
    }

    let parser = sys::MD_PARSER {
        abi_version: 0,
        flags: flags.raw(),
        enter_block: Some(enter_block_cb::<H>),
        leave_block: Some(leave_block_cb::<H>),
        enter_span: Some(enter_span_cb::<H>),
        leave_span: Some(leave_span_cb::<H>),
        text: Some(text_cb::<H>),
        debug_log: None,
        syntax: None,
    };

    let mut ctx = Context {
        handler,
        error: None,
    };

    let result = unsafe {
        sys::md_parse(
            input.as_ptr() as *const sys::MD_CHAR,
            input.len() as sys::MD_SIZE,
            &parser,
            &mut ctx as *mut Context<H> as *mut c_void,
        )
    };

    match result {
        0 => Ok(()),
        -1 => Err(ParseError::RuntimeError),
        code => {
            if let Some(err) = ctx.error {
                Err(ParseError::CallbackError(err))
            } else {
                Err(ParseError::CallbackError(code))
            }
        }
    }
}

/// Parse markdown and collect all events
pub fn parse_to_events(input: &str, flags: ParserFlags) -> ParseResult<Vec<Event>> {
    struct EventCollector {
        events: Vec<Event>,
    }

    impl ParserHandler for EventCollector {
        fn enter_block(&mut self, block: Block) -> bool {
            self.events.push(Event::EnterBlock(block));
            true
        }

        fn leave_block(&mut self, block_type: BlockType) -> bool {
            self.events.push(Event::LeaveBlock(block_type));
            true
        }

        fn enter_span(&mut self, span: Span) -> bool {
            self.events.push(Event::EnterSpan(span));
            true
        }

        fn leave_span(&mut self, span_type: SpanType) -> bool {
            self.events.push(Event::LeaveSpan(span_type));
            true
        }

        fn text(&mut self, text_type: TextType, text: &str) -> bool {
            self.events.push(Event::Text(text_type, text.to_owned()));
            true
        }
    }

    let mut collector = EventCollector { events: Vec::new() };
    parse(input, flags, &mut collector)?;
    Ok(collector.events)
}

// Helper functions to parse detail structures

unsafe fn parse_block(block_type: sys::MD_BLOCKTYPE, detail: *mut c_void) -> Block {
    match block_type {
        sys::MD_BLOCK_DOC => Block::Document,
        sys::MD_BLOCK_QUOTE => Block::Quote,
        sys::MD_BLOCK_UL => {
            let d = &*(detail as *const sys::MD_BLOCK_UL_DETAIL);
            Block::UnorderedList(UnorderedListDetail {
                is_tight: d.is_tight != 0,
                mark: ListMark::from_raw(d.mark),
            })
        }
        sys::MD_BLOCK_OL => {
            let d = &*(detail as *const sys::MD_BLOCK_OL_DETAIL);
            Block::OrderedList(OrderedListDetail {
                start: d.start,
                is_tight: d.is_tight != 0,
                delimiter: OrderedListDelimiter::from_raw(d.mark_delimiter),
            })
        }
        sys::MD_BLOCK_LI => {
            let d = &*(detail as *const sys::MD_BLOCK_LI_DETAIL);
            let task_state = if d.is_task == 0 {
                TaskState::NotTask
            } else {
                match d.task_mark as u8 as char {
                    'x' | 'X' => TaskState::Checked,
                    _ => TaskState::Unchecked,
                }
            };
            Block::ListItem(ListItemDetail { task_state })
        }
        sys::MD_BLOCK_HR => Block::HorizontalRule,
        sys::MD_BLOCK_H => {
            let d = &*(detail as *const sys::MD_BLOCK_H_DETAIL);
            Block::Heading(HeadingDetail {
                level: d.level as u8,
            })
        }
        sys::MD_BLOCK_CODE => {
            let d = &*(detail as *const sys::MD_BLOCK_CODE_DETAIL);
            Block::Code(CodeBlockDetail {
                info: attribute_to_string(&d.info),
                lang: attribute_to_string(&d.lang),
                fence_char: FenceChar::from_raw(d.fence_char),
            })
        }
        sys::MD_BLOCK_HTML => Block::Html,
        sys::MD_BLOCK_P => Block::Paragraph,
        sys::MD_BLOCK_TABLE => {
            let d = &*(detail as *const sys::MD_BLOCK_TABLE_DETAIL);
            Block::Table(TableDetail {
                column_count: d.col_count,
                head_row_count: d.head_row_count,
                body_row_count: d.body_row_count,
            })
        }
        sys::MD_BLOCK_THEAD => Block::TableHead,
        sys::MD_BLOCK_TBODY => Block::TableBody,
        sys::MD_BLOCK_TR => Block::TableRow,
        sys::MD_BLOCK_TH => {
            let d = &*(detail as *const sys::MD_BLOCK_TD_DETAIL);
            Block::TableHeaderCell(TableCellDetail {
                alignment: Alignment::from_raw(d.align),
            })
        }
        sys::MD_BLOCK_TD => {
            let d = &*(detail as *const sys::MD_BLOCK_TD_DETAIL);
            Block::TableCell(TableCellDetail {
                alignment: Alignment::from_raw(d.align),
            })
        }
        _ => Block::Document,
    }
}

unsafe fn parse_span(span_type: sys::MD_SPANTYPE, detail: *mut c_void) -> Span {
    match span_type {
        sys::MD_SPAN_EM => Span::Emphasis,
        sys::MD_SPAN_STRONG => Span::Strong,
        sys::MD_SPAN_A => {
            let d = &*(detail as *const sys::MD_SPAN_A_DETAIL);
            Span::Link(LinkDetail {
                href: attribute_to_string(&d.href),
                title: attribute_to_string(&d.title),
                is_autolink: d.is_autolink != 0,
            })
        }
        sys::MD_SPAN_IMG => {
            let d = &*(detail as *const sys::MD_SPAN_IMG_DETAIL);
            Span::Image(ImageDetail {
                src: attribute_to_string(&d.src),
                title: attribute_to_string(&d.title),
            })
        }
        sys::MD_SPAN_CODE => Span::Code,
        sys::MD_SPAN_DEL => Span::Strikethrough,
        sys::MD_SPAN_LATEXMATH => Span::LatexMath,
        sys::MD_SPAN_LATEXMATH_DISPLAY => Span::LatexMathDisplay,
        sys::MD_SPAN_WIKILINK => {
            let d = &*(detail as *const sys::MD_SPAN_WIKILINK_DETAIL);
            Span::WikiLink(WikiLinkDetail {
                target: attribute_to_string(&d.target),
            })
        }
        sys::MD_SPAN_U => Span::Underline,
        sys::MD_SPAN_SPOILER => Span::Spoiler,
        _ => Span::Emphasis,
    }
}

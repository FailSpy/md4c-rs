//! Raw FFI bindings to the MD4C C library.
//!
//! These bindings are unsafe and should not be used directly.
//! Use the safe wrapper types in the parent module instead.

#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

use std::os::raw::{c_char, c_int, c_uint, c_void};

// Type aliases matching MD4C's types
pub type MD_CHAR = c_char;
pub type MD_SIZE = c_uint;
pub type MD_OFFSET = c_uint;

// Block types
pub type MD_BLOCKTYPE = c_uint;
pub const MD_BLOCK_DOC: MD_BLOCKTYPE = 0;
pub const MD_BLOCK_QUOTE: MD_BLOCKTYPE = 1;
pub const MD_BLOCK_UL: MD_BLOCKTYPE = 2;
pub const MD_BLOCK_OL: MD_BLOCKTYPE = 3;
pub const MD_BLOCK_LI: MD_BLOCKTYPE = 4;
pub const MD_BLOCK_HR: MD_BLOCKTYPE = 5;
pub const MD_BLOCK_H: MD_BLOCKTYPE = 6;
pub const MD_BLOCK_CODE: MD_BLOCKTYPE = 7;
pub const MD_BLOCK_HTML: MD_BLOCKTYPE = 8;
pub const MD_BLOCK_P: MD_BLOCKTYPE = 9;
pub const MD_BLOCK_TABLE: MD_BLOCKTYPE = 10;
pub const MD_BLOCK_THEAD: MD_BLOCKTYPE = 11;
pub const MD_BLOCK_TBODY: MD_BLOCKTYPE = 12;
pub const MD_BLOCK_TR: MD_BLOCKTYPE = 13;
pub const MD_BLOCK_TH: MD_BLOCKTYPE = 14;
pub const MD_BLOCK_TD: MD_BLOCKTYPE = 15;

// Span types
pub type MD_SPANTYPE = c_uint;
pub const MD_SPAN_EM: MD_SPANTYPE = 0;
pub const MD_SPAN_STRONG: MD_SPANTYPE = 1;
pub const MD_SPAN_A: MD_SPANTYPE = 2;
pub const MD_SPAN_IMG: MD_SPANTYPE = 3;
pub const MD_SPAN_CODE: MD_SPANTYPE = 4;
pub const MD_SPAN_DEL: MD_SPANTYPE = 5;
pub const MD_SPAN_LATEXMATH: MD_SPANTYPE = 6;
pub const MD_SPAN_LATEXMATH_DISPLAY: MD_SPANTYPE = 7;
pub const MD_SPAN_WIKILINK: MD_SPANTYPE = 8;
pub const MD_SPAN_U: MD_SPANTYPE = 9;

// Text types
pub type MD_TEXTTYPE = c_uint;
pub const MD_TEXT_NORMAL: MD_TEXTTYPE = 0;
pub const MD_TEXT_NULLCHAR: MD_TEXTTYPE = 1;
pub const MD_TEXT_BR: MD_TEXTTYPE = 2;
pub const MD_TEXT_SOFTBR: MD_TEXTTYPE = 3;
pub const MD_TEXT_ENTITY: MD_TEXTTYPE = 4;
pub const MD_TEXT_CODE: MD_TEXTTYPE = 5;
pub const MD_TEXT_HTML: MD_TEXTTYPE = 6;
pub const MD_TEXT_LATEXMATH: MD_TEXTTYPE = 7;

// Alignment
pub type MD_ALIGN = c_uint;
pub const MD_ALIGN_DEFAULT: MD_ALIGN = 0;
pub const MD_ALIGN_LEFT: MD_ALIGN = 1;
pub const MD_ALIGN_CENTER: MD_ALIGN = 2;
pub const MD_ALIGN_RIGHT: MD_ALIGN = 3;

// Parser flags
pub const MD_FLAG_COLLAPSEWHITESPACE: c_uint = 0x0001;
pub const MD_FLAG_PERMISSIVEATXHEADERS: c_uint = 0x0002;
pub const MD_FLAG_PERMISSIVEURLAUTOLINKS: c_uint = 0x0004;
pub const MD_FLAG_PERMISSIVEEMAILAUTOLINKS: c_uint = 0x0008;
pub const MD_FLAG_NOINDENTEDCODEBLOCKS: c_uint = 0x0010;
pub const MD_FLAG_NOHTMLBLOCKS: c_uint = 0x0020;
pub const MD_FLAG_NOHTMLSPANS: c_uint = 0x0040;
pub const MD_FLAG_TABLES: c_uint = 0x0100;
pub const MD_FLAG_STRIKETHROUGH: c_uint = 0x0200;
pub const MD_FLAG_PERMISSIVEWWWAUTOLINKS: c_uint = 0x0400;
pub const MD_FLAG_TASKLISTS: c_uint = 0x0800;
pub const MD_FLAG_LATEXMATHSPANS: c_uint = 0x1000;
pub const MD_FLAG_WIKILINKS: c_uint = 0x2000;
pub const MD_FLAG_UNDERLINE: c_uint = 0x4000;
pub const MD_FLAG_HARD_SOFT_BREAKS: c_uint = 0x8000;

// Convenience flag combinations
pub const MD_FLAG_PERMISSIVEAUTOLINKS: c_uint =
    MD_FLAG_PERMISSIVEURLAUTOLINKS | MD_FLAG_PERMISSIVEEMAILAUTOLINKS | MD_FLAG_PERMISSIVEWWWAUTOLINKS;
pub const MD_FLAG_NOHTML: c_uint = MD_FLAG_NOHTMLBLOCKS | MD_FLAG_NOHTMLSPANS;

// Dialect presets
pub const MD_DIALECT_COMMONMARK: c_uint = 0;
pub const MD_DIALECT_GITHUB: c_uint =
    MD_FLAG_PERMISSIVEAUTOLINKS | MD_FLAG_TABLES | MD_FLAG_STRIKETHROUGH | MD_FLAG_TASKLISTS;

// HTML renderer flags
pub const MD_HTML_FLAG_DEBUG: c_uint = 0x0001;
pub const MD_HTML_FLAG_VERBATIM_ENTITIES: c_uint = 0x0002;
pub const MD_HTML_FLAG_SKIP_UTF8_BOM: c_uint = 0x0004;
pub const MD_HTML_FLAG_XHTML: c_uint = 0x0008;

/// Attribute structure for links, images, etc.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD_ATTRIBUTE {
    pub text: *const MD_CHAR,
    pub size: MD_SIZE,
    pub substr_types: *const MD_TEXTTYPE,
    pub substr_offsets: *const MD_OFFSET,
}

/// Detail for unordered lists
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD_BLOCK_UL_DETAIL {
    pub is_tight: c_int,
    pub mark: c_char,
}

/// Detail for ordered lists
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD_BLOCK_OL_DETAIL {
    pub start: c_uint,
    pub is_tight: c_int,
    pub mark_delimiter: c_char,
}

/// Detail for list items
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD_BLOCK_LI_DETAIL {
    pub is_task: c_int,
    pub task_mark: c_char,
    pub task_mark_offset: MD_OFFSET,
}

/// Detail for headers
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD_BLOCK_H_DETAIL {
    pub level: c_uint,
}

/// Detail for code blocks
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD_BLOCK_CODE_DETAIL {
    pub info: MD_ATTRIBUTE,
    pub lang: MD_ATTRIBUTE,
    pub fence_char: c_char,
}

/// Detail for tables
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD_BLOCK_TABLE_DETAIL {
    pub col_count: c_uint,
    pub head_row_count: c_uint,
    pub body_row_count: c_uint,
}

/// Detail for table cells (th/td)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD_BLOCK_TD_DETAIL {
    pub align: MD_ALIGN,
}

/// Detail for links
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD_SPAN_A_DETAIL {
    pub href: MD_ATTRIBUTE,
    pub title: MD_ATTRIBUTE,
    pub is_autolink: c_int,
}

/// Detail for images
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD_SPAN_IMG_DETAIL {
    pub src: MD_ATTRIBUTE,
    pub title: MD_ATTRIBUTE,
}

/// Detail for wiki links
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD_SPAN_WIKILINK_DETAIL {
    pub target: MD_ATTRIBUTE,
}

/// Callback function types
pub type EnterBlockFn = unsafe extern "C" fn(MD_BLOCKTYPE, *mut c_void, *mut c_void) -> c_int;
pub type LeaveBlockFn = unsafe extern "C" fn(MD_BLOCKTYPE, *mut c_void, *mut c_void) -> c_int;
pub type EnterSpanFn = unsafe extern "C" fn(MD_SPANTYPE, *mut c_void, *mut c_void) -> c_int;
pub type LeaveSpanFn = unsafe extern "C" fn(MD_SPANTYPE, *mut c_void, *mut c_void) -> c_int;
pub type TextFn = unsafe extern "C" fn(MD_TEXTTYPE, *const MD_CHAR, MD_SIZE, *mut c_void) -> c_int;
pub type DebugLogFn = unsafe extern "C" fn(*const c_char, *mut c_void);

/// Parser configuration structure
#[repr(C)]
pub struct MD_PARSER {
    pub abi_version: c_uint,
    pub flags: c_uint,
    pub enter_block: Option<EnterBlockFn>,
    pub leave_block: Option<LeaveBlockFn>,
    pub enter_span: Option<EnterSpanFn>,
    pub leave_span: Option<LeaveSpanFn>,
    pub text: Option<TextFn>,
    pub debug_log: Option<DebugLogFn>,
    pub syntax: Option<unsafe extern "C" fn()>,
}

extern "C" {
    /// Parse markdown text and invoke callbacks
    pub fn md_parse(
        text: *const MD_CHAR,
        size: MD_SIZE,
        parser: *const MD_PARSER,
        userdata: *mut c_void,
    ) -> c_int;
}

#[cfg(feature = "html")]
pub type HtmlProcessOutputFn = unsafe extern "C" fn(*const MD_CHAR, MD_SIZE, *mut c_void);

#[cfg(feature = "html")]
extern "C" {
    /// Render markdown to HTML
    pub fn md_html(
        input: *const MD_CHAR,
        input_size: MD_SIZE,
        process_output: Option<HtmlProcessOutputFn>,
        userdata: *mut c_void,
        parser_flags: c_uint,
        renderer_flags: c_uint,
    ) -> c_int;
}

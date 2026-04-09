//! HTML rendering functionality.

use crate::parser::ParserFlags;
use crate::sys;
use std::os::raw::c_void;

/// HTML renderer configuration flags
#[derive(Debug, Clone, Copy, Default)]
pub struct HtmlFlags {
    flags: u32,
}

impl HtmlFlags {
    /// Create empty flags
    pub const fn new() -> Self {
        HtmlFlags { flags: 0 }
    }

    /// Enable debug output to stderr
    pub const fn debug(mut self) -> Self {
        self.flags |= sys::MD_HTML_FLAG_DEBUG;
        self
    }

    /// Don't translate HTML entities
    pub const fn verbatim_entities(mut self) -> Self {
        self.flags |= sys::MD_HTML_FLAG_VERBATIM_ENTITIES;
        self
    }

    /// Skip UTF-8 BOM if present
    pub const fn skip_utf8_bom(mut self) -> Self {
        self.flags |= sys::MD_HTML_FLAG_SKIP_UTF8_BOM;
        self
    }

    /// Output XHTML-compliant HTML
    pub const fn xhtml(mut self) -> Self {
        self.flags |= sys::MD_HTML_FLAG_XHTML;
        self
    }

    /// Get the raw flags value
    pub const fn raw(self) -> u32 {
        self.flags
    }
}

/// Error type for HTML rendering
#[derive(Debug, Clone)]
pub enum HtmlError {
    /// Renderer encountered an error
    RenderError,
}

impl std::fmt::Display for HtmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HtmlError::RenderError => write!(f, "HTML rendering error"),
        }
    }
}

impl std::error::Error for HtmlError {}

/// Result type for HTML operations
pub type HtmlResult<T> = Result<T, HtmlError>;

/// Render markdown to HTML string
///
/// # Arguments
/// * `input` - Markdown source text
/// * `parser_flags` - Parser configuration flags
/// * `html_flags` - HTML renderer configuration flags
///
/// # Returns
/// * `Ok(String)` - The rendered HTML
/// * `Err(HtmlError)` - If rendering failed
///
/// # Example
/// ```
/// use md4c::html::{render_html, HtmlFlags};
/// use md4c::parser::ParserFlags;
///
/// let markdown = "# Hello\n\nThis is **bold** text.";
/// let html = render_html(markdown, ParserFlags::commonmark(), HtmlFlags::new()).unwrap();
/// assert!(html.contains("<h1>Hello</h1>"));
/// assert!(html.contains("<strong>bold</strong>"));
/// ```
pub fn render_html(
    input: &str,
    parser_flags: ParserFlags,
    html_flags: HtmlFlags,
) -> HtmlResult<String> {
    struct OutputBuffer {
        buffer: String,
    }

    unsafe extern "C" fn output_callback(
        text: *const sys::MD_CHAR,
        size: sys::MD_SIZE,
        userdata: *mut c_void,
    ) {
        let buf = &mut *(userdata as *mut OutputBuffer);
        let slice = std::slice::from_raw_parts(text as *const u8, size as usize);
        if let Ok(s) = std::str::from_utf8(slice) {
            buf.buffer.push_str(s);
        }
    }

    let mut output = OutputBuffer {
        buffer: String::with_capacity(input.len() * 2),
    };

    let result = unsafe {
        sys::md_html(
            input.as_ptr() as *const sys::MD_CHAR,
            input.len() as sys::MD_SIZE,
            Some(output_callback),
            &mut output as *mut OutputBuffer as *mut c_void,
            parser_flags.raw(),
            html_flags.raw(),
        )
    };

    if result == 0 {
        Ok(output.buffer)
    } else {
        Err(HtmlError::RenderError)
    }
}

/// Render markdown to HTML with streaming output
///
/// This function is useful for large documents where you want to process
/// output incrementally rather than accumulating it in memory.
///
/// # Arguments
/// * `input` - Markdown source text
/// * `parser_flags` - Parser configuration flags
/// * `html_flags` - HTML renderer configuration flags
/// * `callback` - Function called with each chunk of HTML output
///
/// # Example
/// ```
/// use md4c::html::{render_html_streaming, HtmlFlags};
/// use md4c::parser::ParserFlags;
/// use std::io::Write;
///
/// let markdown = "# Hello World";
/// let mut output = Vec::new();
/// render_html_streaming(
///     markdown,
///     ParserFlags::commonmark(),
///     HtmlFlags::new(),
///     |chunk| {
///         output.extend_from_slice(chunk.as_bytes());
///     }
/// ).unwrap();
/// ```
pub fn render_html_streaming<F>(
    input: &str,
    parser_flags: ParserFlags,
    html_flags: HtmlFlags,
    mut callback: F,
) -> HtmlResult<()>
where
    F: FnMut(&str),
{
    struct CallbackContext<'a, F: FnMut(&str)> {
        callback: &'a mut F,
    }

    unsafe extern "C" fn output_callback<F: FnMut(&str)>(
        text: *const sys::MD_CHAR,
        size: sys::MD_SIZE,
        userdata: *mut c_void,
    ) {
        let ctx = &mut *(userdata as *mut CallbackContext<F>);
        let slice = std::slice::from_raw_parts(text as *const u8, size as usize);
        if let Ok(s) = std::str::from_utf8(slice) {
            (ctx.callback)(s);
        }
    }

    let mut ctx = CallbackContext {
        callback: &mut callback,
    };

    let result = unsafe {
        sys::md_html(
            input.as_ptr() as *const sys::MD_CHAR,
            input.len() as sys::MD_SIZE,
            Some(output_callback::<F>),
            &mut ctx as *mut CallbackContext<F> as *mut c_void,
            parser_flags.raw(),
            html_flags.raw(),
        )
    };

    if result == 0 {
        Ok(())
    } else {
        Err(HtmlError::RenderError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_html_rendering() {
        let html = render_html("Hello **world**", ParserFlags::commonmark(), HtmlFlags::new())
            .unwrap();
        assert!(html.contains("<strong>world</strong>"));
    }

    #[test]
    fn test_heading() {
        let html = render_html("# Title", ParserFlags::commonmark(), HtmlFlags::new()).unwrap();
        assert!(html.contains("<h1>Title</h1>"));
    }

    #[test]
    fn test_github_flavor() {
        let html = render_html(
            "~~strikethrough~~",
            ParserFlags::github(),
            HtmlFlags::new(),
        )
        .unwrap();
        assert!(html.contains("<del>strikethrough</del>"));
    }

    #[test]
    fn test_xhtml_output() {
        let html = render_html("line1  \nline2", ParserFlags::commonmark(), HtmlFlags::new().xhtml())
            .unwrap();
        assert!(html.contains("<br />"));
    }
}

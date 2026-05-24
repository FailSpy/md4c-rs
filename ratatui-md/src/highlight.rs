//! Syntax highlighting for code blocks.
//!
//! This module provides syntax highlighting using syntect when the
//! `syntect` feature is enabled.
//!
//! Step 3b adds a `highlight_with_offsets` API that returns parallel
//! byte-offset side-channel data alongside each emitted Span, so
//! downstream consumers (Cadenza's selection engine) can recover the
//! source byte range for every grapheme inside a highlighted code block.
//! The pointer arithmetic is `usize`-based with both-way bounds checks
//! so the path is UB-free.

use ratatui::text::Span;

/// A highlighted code line WITH a parallel byte-offset side-channel.
///
/// `spans[i]` is the styled span; `span_offsets[i]` is the absolute byte
/// range in the original `code` string that produced that span — or
/// `None` for synthetic spans where pointer arithmetic isn't valid.
#[derive(Debug, Clone)]
pub struct HighlightedLine {
    /// Byte offset within the original `code` where this line begins.
    /// Used as the base for per-grapheme offset reconstruction.
    pub line_byte_start: u32,
    pub spans: Vec<Span<'static>>,
    pub span_offsets: Vec<Option<(u32, u32)>>,
}

#[cfg(feature = "syntect")]
mod syntect_impl {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use std::sync::OnceLock;
    use syntect::easy::HighlightLines;
    use syntect::highlighting::{FontStyle, ThemeSet};
    use syntect::parsing::SyntaxSet;
    use syntect::util::LinesWithEndings;

    /// Compute the byte range of a span-text slice within the original
    /// `code` string via `usize` arithmetic with checked_add both ways +
    /// `is_char_boundary` validation. Returns `None` if the text pointer
    /// lies outside the input buffer or if address arithmetic would
    /// overflow.
    fn compute_offset_checked(
        text_start: usize,
        text_len: usize,
        code_start: usize,
        code_end: usize,
        code: &str,
    ) -> Option<(u32, u32)> {
        let text_end = text_start.checked_add(text_len)?;
        if text_start < code_start || text_end > code_end {
            return None;
        }
        let start_rel = text_start - code_start;
        let end_rel = text_end - code_start;
        if start_rel > u32::MAX as usize || end_rel > u32::MAX as usize {
            return None;
        }
        if !code.is_char_boundary(start_rel) || !code.is_char_boundary(end_rel) {
            return None;
        }
        Some((start_rel as u32, end_rel as u32))
    }

    /// Global syntax/theme sets - loaded once, reused forever
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

    fn get_syntax_set() -> &'static SyntaxSet {
        SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
    }

    fn get_theme_set() -> &'static ThemeSet {
        THEME_SET.get_or_init(ThemeSet::load_defaults)
    }

    /// Syntax highlighter using syntect.
    ///
    /// Uses global static syntax/theme sets for efficiency - creating
    /// multiple SyntaxHighlighter instances is cheap.
    pub struct SyntaxHighlighter {
        theme_name: String,
    }

    impl Default for SyntaxHighlighter {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SyntaxHighlighter {
        /// Create a new syntax highlighter with default themes.
        ///
        /// This is cheap - syntax definitions are loaded once globally.
        pub fn new() -> Self {
            // Ensure global sets are initialized
            let _ = get_syntax_set();
            let _ = get_theme_set();
            Self {
                theme_name: "base16-ocean.dark".to_string(),
            }
        }

        /// Set the theme by name.
        ///
        /// Available themes: "base16-ocean.dark", "base16-eighties.dark",
        /// "base16-mocha.dark", "base16-ocean.light", "InspiredGitHub",
        /// "Solarized (dark)", "Solarized (light)"
        pub fn theme(mut self, name: &str) -> Self {
            if get_theme_set().themes.contains_key(name) {
                self.theme_name = name.to_string();
            }
            self
        }

        /// List available theme names.
        pub fn available_themes(&self) -> Vec<&'static str> {
            get_theme_set().themes.keys().map(|s| s.as_str()).collect()
        }

        /// List available syntax names.
        pub fn available_syntaxes(&self) -> Vec<&'static str> {
            get_syntax_set()
                .syntaxes()
                .iter()
                .map(|s| s.name.as_str())
                .collect()
        }

        /// Highlight code and return ratatui Lines.
        ///
        /// # Arguments
        /// * `code` - The source code to highlight
        /// * `language` - The language name (e.g., "rust", "python", "javascript")
        ///
        /// # Returns
        /// A vector of Lines with syntax highlighting applied.
        pub fn highlight(&self, code: &str, language: &str) -> Vec<Line<'static>> {
            let syntax_set = get_syntax_set();
            let theme_set = get_theme_set();

            let syntax = syntax_set
                .find_syntax_by_token(language)
                .or_else(|| syntax_set.find_syntax_by_extension(language))
                .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

            let theme = theme_set.themes.get(&self.theme_name).unwrap_or_else(|| {
                theme_set
                    .themes
                    .values()
                    .next()
                    .expect("No themes available")
            });

            let mut highlighter = HighlightLines::new(syntax, theme);
            let mut lines = Vec::new();

            for line in LinesWithEndings::from(code) {
                let ranges = highlighter
                    .highlight_line(line, syntax_set)
                    .unwrap_or_default();

                let spans: Vec<Span<'static>> = ranges
                    .into_iter()
                    .map(|(style, text)| {
                        let fg =
                            Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);

                        let mut ratatui_style = Style::default().fg(fg);

                        if style.font_style.contains(FontStyle::BOLD) {
                            ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
                        }
                        if style.font_style.contains(FontStyle::ITALIC) {
                            ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
                        }
                        if style.font_style.contains(FontStyle::UNDERLINE) {
                            ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
                        }

                        Span::styled(text.trim_end_matches('\n').to_string(), ratatui_style)
                    })
                    .collect();

                lines.push(Line::from(spans));
            }

            lines
        }

        /// Highlight code with a specific background color.
        pub fn highlight_with_background(
            &self,
            code: &str,
            language: &str,
            bg: Color,
        ) -> Vec<Line<'static>> {
            let mut lines = self.highlight(code, language);
            for line in &mut lines {
                for span in line.spans.iter_mut() {
                    span.style = span.style.bg(bg);
                }
            }
            lines
        }

        /// Highlight code AND return parallel byte-offset side-channel so
        /// callers can recover the source byte range for every emitted
        /// span (and thus every grapheme within it).
        ///
        /// The byte offset is computed BEFORE the `.to_string()` clone via
        /// pointer arithmetic on the syntect-returned `&str` (which is a
        /// slice of the line slice, which is a slice of the input `code`).
        /// All arithmetic is `usize`-based with both-way bounds checks so
        /// the path is UB-free.
        pub fn highlight_with_offsets(
            &self,
            code: &str,
            language: &str,
        ) -> Vec<crate::highlight::HighlightedLine> {
            use crate::highlight::HighlightedLine;

            let syntax_set = get_syntax_set();
            let theme_set = get_theme_set();

            let syntax = syntax_set
                .find_syntax_by_token(language)
                .or_else(|| syntax_set.find_syntax_by_extension(language))
                .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

            let theme = theme_set.themes.get(&self.theme_name).unwrap_or_else(|| {
                theme_set
                    .themes
                    .values()
                    .next()
                    .expect("No themes available")
            });

            let mut highlighter = HighlightLines::new(syntax, theme);
            let mut lines = Vec::new();

            // Track byte cursor through `code` so we can compute absolute
            // byte offsets for every (sub)slice the syntect highlighter
            // returns. LinesWithEndings yields each line WITH its trailing
            // newline (if any); we advance by line.len() per iteration.
            //
            // checked_add (not wrapping_add) so a `code.len()` near
            // usize::MAX cannot smuggle a malformed range past the
            // `text_end <= code_end` check. Mirrors the Step 2
            // md4c-rs pointer-arithmetic safety policy.
            let code_start = code.as_ptr() as usize;
            let code_end = match code_start.checked_add(code.len()) {
                Some(end) => end,
                None => return Vec::new(), // pathological: address-space overflow
            };
            let mut line_byte_offset: usize = 0;

            for line in LinesWithEndings::from(code) {
                let line_start_in_code = line_byte_offset;
                let ranges = highlighter
                    .highlight_line(line, syntax_set)
                    .unwrap_or_default();

                let mut spans: Vec<Span<'static>> = Vec::with_capacity(ranges.len());
                let mut span_offsets: Vec<Option<(u32, u32)>> =
                    Vec::with_capacity(ranges.len());

                for (style, text) in ranges {
                    // Pointer arithmetic in usize with checked_add both ways
                    // BEFORE any clone of `text`. If syntect returned a
                    // slice that isn't inside `code` (shouldn't happen, but
                    // defensive), or if address arithmetic would overflow,
                    // the offset is None.
                    let text_start = text.as_ptr() as usize;
                    let abs_offset = compute_offset_checked(
                        text_start,
                        text.len(),
                        code_start,
                        code_end,
                        code,
                    );

                    // Note: `text.trim_end_matches('\n')` may produce a
                    // shorter logical string but the absolute byte offset
                    // we recorded is the PRE-trim byte range — so the
                    // caller using these offsets to grapheme-walk should
                    // index back into the trim'd string. We record the
                    // POST-trim offset for safer round-trip:
                    let trimmed = text.trim_end_matches('\n');
                    let trim_offset = abs_offset.map(|(s, e)| {
                        let trim_len_diff = (text.len() - trimmed.len()) as u32;
                        (s, e.saturating_sub(trim_len_diff))
                    });

                    let fg =
                        Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                    let mut ratatui_style = Style::default().fg(fg);
                    if style.font_style.contains(FontStyle::BOLD) {
                        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
                    }
                    if style.font_style.contains(FontStyle::ITALIC) {
                        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
                    }
                    if style.font_style.contains(FontStyle::UNDERLINE) {
                        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
                    }

                    spans.push(Span::styled(trimmed.to_string(), ratatui_style));
                    span_offsets.push(trim_offset);
                }

                lines.push(HighlightedLine {
                    line_byte_start: line_start_in_code as u32,
                    spans,
                    span_offsets,
                });
                line_byte_offset += line.len();
            }

            lines
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_highlight_rust() {
            let highlighter = SyntaxHighlighter::new();
            let code = "fn main() {\n    println!(\"Hello\");\n}";
            let lines = highlighter.highlight(code, "rust");
            assert_eq!(lines.len(), 3);
        }

        #[test]
        fn test_highlight_unknown_language() {
            let highlighter = SyntaxHighlighter::new();
            let code = "some text";
            let lines = highlighter.highlight(code, "nonexistent");
            assert_eq!(lines.len(), 1);
        }

        #[test]
        fn test_available_themes() {
            let highlighter = SyntaxHighlighter::new();
            let themes = highlighter.available_themes();
            assert!(!themes.is_empty());
        }
    }
}

#[cfg(feature = "syntect")]
pub use syntect_impl::SyntaxHighlighter;

/// Placeholder for when syntect is not enabled.
#[cfg(not(feature = "syntect"))]
pub struct SyntaxHighlighter;

#[cfg(not(feature = "syntect"))]
impl SyntaxHighlighter {
    /// Create a new syntax highlighter (no-op without syntect feature).
    pub fn new() -> Self {
        Self
    }

    /// Set the theme (no-op without syntect feature).
    pub fn theme(self, _name: &str) -> Self {
        self
    }

    /// Highlight code (returns plain text without syntect feature).
    pub fn highlight(&self, code: &str, _language: &str) -> Vec<ratatui::text::Line<'static>> {
        code.lines()
            .map(|line| ratatui::text::Line::raw(line.to_string()))
            .collect()
    }

    /// Highlight code with a background color (returns plain text without syntect feature).
    pub fn highlight_with_background(
        &self,
        code: &str,
        language: &str,
        _bg: ratatui::style::Color,
    ) -> Vec<ratatui::text::Line<'static>> {
        self.highlight(code, language)
    }

    /// Highlight with parallel byte-offset side-channel (no-syntect path).
    ///
    /// Without syntect, the whole line becomes a single un-styled Span.
    /// The byte offset is just the line's position within `code`.
    pub fn highlight_with_offsets(
        &self,
        code: &str,
        _language: &str,
    ) -> Vec<crate::highlight::HighlightedLine> {
        use crate::highlight::HighlightedLine;
        let mut out = Vec::new();
        let mut offset: usize = 0;
        for line in code.split_inclusive('\n') {
            let line_start = offset;
            let line_str = line.trim_end_matches('\n');
            let len = line_str.len();
            let end = line_start + len;
            out.push(HighlightedLine {
                line_byte_start: line_start as u32,
                spans: vec![ratatui::text::Span::raw(line_str.to_string())],
                span_offsets: vec![Some((line_start as u32, end as u32))],
            });
            offset += line.len();
        }
        if out.is_empty() && !code.is_empty() {
            // No-newline code: one line covers the whole code.
            out.push(HighlightedLine {
                line_byte_start: 0,
                spans: vec![ratatui::text::Span::raw(code.to_string())],
                span_offsets: vec![Some((0, code.len() as u32))],
            });
        }
        out
    }
}

#[cfg(not(feature = "syntect"))]
impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

//! Syntax highlighting for code blocks.
//!
//! This module provides syntax highlighting using syntect when the
//! `syntect` feature is enabled.

#[cfg(feature = "syntect")]
mod syntect_impl {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use std::sync::OnceLock;
    use syntect::easy::HighlightLines;
    use syntect::highlighting::{FontStyle, ThemeSet};
    use syntect::parsing::SyntaxSet;
    use syntect::util::LinesWithEndings;

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

            let theme = theme_set
                .themes
                .get(&self.theme_name)
                .unwrap_or_else(|| {
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
                        let fg = Color::Rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        );

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
}

#[cfg(not(feature = "syntect"))]
impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

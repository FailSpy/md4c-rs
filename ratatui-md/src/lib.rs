//! # ratatui-md
//!
//! Markdown rendering for [ratatui](https://github.com/ratatui-org/ratatui) terminal UIs.
//!
//! This crate provides widgets and utilities for rendering Markdown documents
//! in terminal applications built with ratatui. It uses [MD4C](https://github.com/mity/md4c)
//! for fast, CommonMark-compliant parsing.
//!
//! ## Features
//!
//! - **Full Markdown Support**: Headings, emphasis, links, code blocks, lists, tables, etc.
//! - **GitHub Flavored Markdown**: Tables, task lists, strikethrough, autolinks
//! - **Customizable Themes**: Built-in themes or create your own
//! - **Interactive Widgets**: Scrolling, link navigation, heading jumping
//! - **Syntax Highlighting**: Optional code block highlighting via syntect
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ratatui_md::Markdown;
//!
//! let markdown = "# Hello World\n\nThis is **bold** text.\n\n- Item 1\n- Item 2";
//!
//! // Create a widget
//! let widget = Markdown::new(markdown);
//!
//! // Render in your ratatui app
//! // frame.render_widget(widget, area);
//! ```
//!
//! ## Themes
//!
//! ```rust
//! use ratatui_md::{Markdown, Theme};
//!
//! // Use a built-in theme
//! let widget = Markdown::new("# Hello").theme(Theme::dark());
//!
//! // Or customize
//! use ratatui::style::{Color, Style};
//! let mut theme = Theme::default();
//! theme.heading1 = Style::new().fg(Color::Magenta);
//! let widget = Markdown::new("# Hello").theme(theme);
//! ```
//!
//! ## Interactive Viewing
//!
//! For scrollable, interactive markdown documents:
//!
//! ```rust
//! use ratatui_md::MarkdownView;
//!
//! let mut view = MarkdownView::new("# Doc\n\nLong content...");
//!
//! // Scroll
//! view.scroll_down(5);
//! view.scroll_up(2);
//!
//! // Navigate headings
//! let headings = view.headings();
//! view.scroll_to_heading(0);
//!
//! // Navigate links
//! view.select_next_link();
//! if let Some(link) = view.selected_link() {
//!     println!("Selected: {}", link.url);
//! }
//! ```
//!
//! ## Render Options
//!
//! ```rust
//! use ratatui_md::{Markdown, RenderOptions};
//! use md4c::ParserFlags;
//!
//! let options = RenderOptions::default()
//!     .with_width(80)
//!     .with_parser_flags(ParserFlags::github());
//!
//! let widget = Markdown::new("# Hello").options(options);
//! ```
//!
//! ## Direct Text Rendering
//!
//! For more control, render directly to ratatui `Text`:
//!
//! ```rust
//! use ratatui_md::{render, Theme, RenderOptions};
//!
//! let result = render("**bold**", &Theme::default(), &RenderOptions::default());
//! let text = result.text;
//! let links = result.links;
//! let headings = result.headings;
//! ```
//!
//! ## Syntax Highlighting
//!
//! Enable the `syntect` feature for code block highlighting:
//!
//! ```toml
//! [dependencies]
//! ratatui-md = { version = "0.1", features = ["syntect"] }
//! ```
//!
//! ```rust,ignore
//! use ratatui_md::SyntaxHighlighter;
//!
//! let highlighter = SyntaxHighlighter::new().theme("base16-ocean.dark");
//! let lines = highlighter.highlight("fn main() {}", "rust");
//! ```

pub mod highlight;
pub mod latex;
pub mod position_map;
pub mod renderer;
pub mod theme;
pub mod widget;

// Re-export main types
pub use highlight::SyntaxHighlighter;
pub use latex::latex_to_unicode;
pub use position_map::{CharMapping, FormatMark, PositionMap};
pub use renderer::{
    render, render_default, HeadingInfo, LinkInfo, RenderOptions, RenderedMarkdown,
};
pub use theme::Theme;
pub use widget::{Markdown, MarkdownSpan, MarkdownView, MarkdownViewWidget};

// Re-export md4c types that users might need
pub use md4c::ParserFlags;

/// Convenience function to render markdown to ratatui Text.
///
/// Uses default theme and options.
///
/// # Example
///
/// ```
/// let text = ratatui_md::to_text("# Hello **world**");
/// ```
pub fn to_text(markdown: &str) -> ratatui::text::Text<'static> {
    render_default(markdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_text() {
        let text = to_text("# Hello");
        assert!(!text.lines.is_empty());
    }

    #[test]
    fn test_render_with_theme() {
        let theme = Theme::dark();
        let result = render("**bold**", &theme, &RenderOptions::default());
        assert!(!result.text.lines.is_empty());
    }

    #[test]
    fn test_markdown_widget() {
        let _widget = Markdown::new("# Test");
    }

    #[test]
    fn test_list_tight() {
        // Test tight list (no blank lines between items)
        let md = "- Item 1\n- Item 2\n- Item 3";

        let theme = Theme::dark();
        let options = RenderOptions::github().with_width(80);
        let result = render(md, &theme, &options);

        let all_text: String = result
            .text
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            all_text.contains("• Item 1"),
            "Should contain bullet for Item 1"
        );
        assert!(
            all_text.contains("• Item 2"),
            "Should contain bullet for Item 2"
        );
        assert!(
            all_text.contains("• Item 3"),
            "Should contain bullet for Item 3"
        );
    }

    #[test]
    fn test_list_loose() {
        // Test loose list (blank lines between items - paragraphs inside)
        let md = "- Item 1\n\n- Item 2\n\n- Item 3";

        let theme = Theme::dark();
        let options = RenderOptions::github().with_width(80);
        let result = render(md, &theme, &options);

        let all_text: String = result
            .text
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            all_text.contains("• Item 1"),
            "Should contain bullet for Item 1"
        );
        assert!(
            all_text.contains("• Item 2"),
            "Should contain bullet for Item 2"
        );
        assert!(
            all_text.contains("• Item 3"),
            "Should contain bullet for Item 3"
        );
    }

    #[test]
    fn test_nested_list() {
        // Test that nested lists render on separate lines with proper indentation
        let md = "- Item 1\n- Item 2\n  - Nested 2.1\n  - Nested 2.2\n- Item 3";

        let theme = Theme::dark();
        let options = RenderOptions::github().with_width(80);
        let result = render(md, &theme, &options);

        // Collect each line as a separate string
        let lines: Vec<String> = result
            .text
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
            })
            .collect();

        // Each list item should be on its own line - this is the critical bug fix:
        // Previously "Item 2" and "Nested 2.1" would appear on the same line
        assert!(
            lines.iter().any(|l| l.contains("Item 1")),
            "Should have Item 1"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("Item 2") && !l.contains("Nested")),
            "Item 2 should be on its own line without nested items. Lines: {:?}",
            lines
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("Nested 2.1") && !l.contains("Item 2")),
            "Nested 2.1 should be on its own line without parent. Lines: {:?}",
            lines
        );
        assert!(
            lines.iter().any(|l| l.contains("Nested 2.2")),
            "Should have Nested 2.2"
        );
        assert!(
            lines.iter().any(|l| l.contains("Item 3")),
            "Should have Item 3"
        );

        // Nested items should be indented (start with spaces before the bullet)
        let nested_line = lines.iter().find(|l| l.contains("Nested 2.1")).unwrap();
        assert!(
            nested_line.starts_with("  "),
            "Nested items should be indented with 2 spaces, got: {:?}",
            nested_line
        );
    }

    #[test]
    fn test_nested_ordered_list() {
        // Test that nested ordered lists render on separate lines
        let md = "1. First\n2. Second\n   1. Nested 2.1\n   2. Nested 2.2\n3. Third";

        let theme = Theme::dark();
        let options = RenderOptions::github().with_width(80);
        let result = render(md, &theme, &options);

        // Collect each line as a separate string
        let lines: Vec<String> = result
            .text
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
            })
            .collect();

        // Each list item should be on its own line
        assert!(
            lines
                .iter()
                .any(|l| l.contains("Second") && !l.contains("Nested")),
            "Second should be on its own line without nested items: {:?}",
            lines
        );
        assert!(
            lines.iter().any(|l| l.contains("Nested 2.1")),
            "Should have Nested 2.1"
        );
    }

    #[test]
    fn test_list_after_text() {
        // Test that a list after introductory text renders with bullets
        let md = "## Project Structure\n\nSource code: src/ directory with modules for:\n\n- Main application (main.rs, app.rs)\n- Conversation handling (conversation/ module)";

        let theme = Theme::dark();
        let options = RenderOptions::github()
            .with_width(80)
            .with_hard_breaks(true);
        let result = render(md, &theme, &options);

        let all_text: String = result
            .text
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            all_text.contains("• Main application"),
            "Should contain bullet for Main application"
        );
        assert!(
            all_text.contains("• Conversation handling"),
            "Should contain bullet for Conversation handling"
        );
    }

    #[test]
    fn test_code_block_with_newlines() {
        // Test that code blocks properly split on newlines
        let md = "```\nline1\nline2\nline3\n```";

        let theme = Theme::dark();
        let options = RenderOptions::github().with_width(80);
        let result = render(md, &theme, &options);

        // Each line should be on a separate Line in the output
        let line_count = result.text.lines.len();
        assert!(
            line_count >= 3,
            "Should have at least 3 lines, got {}",
            line_count
        );

        // Check content
        let all_text: String = result
            .text
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(all_text.contains("line1"), "Should contain line1");
        assert!(all_text.contains("line2"), "Should contain line2");
        assert!(all_text.contains("line3"), "Should contain line3");
    }

    #[test]
    fn test_tree_in_code_block() {
        // Test that tree structures in code blocks render properly
        let md = "```\nproject/\n├── src/\n│   └── main.rs\n└── Cargo.toml\n```";

        let theme = Theme::dark();
        let options = RenderOptions::github().with_width(80);
        let result = render(md, &theme, &options);

        // Each tree line should be on a separate Line
        let line_count = result.text.lines.len();
        assert!(
            line_count >= 4,
            "Should have at least 4 lines for tree, got {}",
            line_count
        );
    }
}

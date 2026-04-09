//! # MD4C - Rust Bindings
//!
//! This crate provides safe Rust bindings for [MD4C](https://github.com/mity/md4c),
//! a fast CommonMark-compliant Markdown parser written in C.
//!
//! ## Features
//!
//! - **Fast**: MD4C is one of the fastest Markdown parsers available
//! - **CommonMark compliant**: Fully compliant with CommonMark 0.31
//! - **Extensions**: Support for GitHub Flavored Markdown extensions:
//!   - Tables
//!   - Strikethrough
//!   - Task lists
//!   - Autolinks
//!   - Wiki links
//!   - LaTeX math spans
//!   - Underline
//!
//! ## Quick Start
//!
//! ### Render Markdown to HTML
//!
//! ```rust
//! use md4c::{render_html, ParserFlags, HtmlFlags};
//!
//! let markdown = r#"
//! # Hello World
//!
//! This is a **bold** statement with some `code`.
//!
//! - Item 1
//! - Item 2
//! - Item 3
//! "#;
//!
//! let html = render_html(markdown, ParserFlags::commonmark(), HtmlFlags::new()).unwrap();
//! println!("{}", html);
//! ```
//!
//! ### Parse with Custom Handler
//!
//! ```rust
//! use md4c::{parse, ParserFlags, ParserHandler, Block, Span, BlockType, SpanType, TextType};
//!
//! struct MyHandler {
//!     output: String,
//! }
//!
//! impl ParserHandler for MyHandler {
//!     fn enter_block(&mut self, block: Block) -> bool {
//!         match block {
//!             Block::Heading(detail) => {
//!                 self.output.push_str(&format!("[H{}]", detail.level));
//!             }
//!             Block::Paragraph => {
//!                 self.output.push_str("[P]");
//!             }
//!             _ => {}
//!         }
//!         true
//!     }
//!
//!     fn leave_block(&mut self, block_type: BlockType) -> bool {
//!         match block_type {
//!             BlockType::Heading => self.output.push_str("[/H]"),
//!             BlockType::Paragraph => self.output.push_str("[/P]"),
//!             _ => {}
//!         }
//!         true
//!     }
//!
//!     fn text(&mut self, _text_type: TextType, text: &str) -> bool {
//!         self.output.push_str(text);
//!         true
//!     }
//! }
//!
//! let mut handler = MyHandler { output: String::new() };
//! parse("# Title\n\nParagraph", ParserFlags::commonmark(), &mut handler).unwrap();
//! assert!(handler.output.contains("[H1]Title[/H]"));
//! ```
//!
//! ### GitHub Flavored Markdown
//!
//! ```rust
//! use md4c::{render_html, ParserFlags, HtmlFlags};
//!
//! let gfm = r#"
//! | Header 1 | Header 2 |
//! |----------|----------|
//! | Cell 1   | Cell 2   |
//!
//! - [x] Task 1 (done)
//! - [ ] Task 2 (pending)
//!
//! ~~strikethrough~~
//! "#;
//!
//! let html = render_html(gfm, ParserFlags::github(), HtmlFlags::new()).unwrap();
//! ```
//!
//! ## Parser Flags
//!
//! Parser behavior can be customized using `ParserFlags`:
//!
//! ```rust
//! use md4c::ParserFlags;
//!
//! // CommonMark (default, no extensions)
//! let flags = ParserFlags::commonmark();
//!
//! // GitHub Flavored Markdown
//! let flags = ParserFlags::github();
//!
//! // Custom configuration
//! let flags = ParserFlags::new()
//!     .tables()
//!     .strikethrough()
//!     .task_lists()
//!     .no_html()  // Disable HTML passthrough
//!     .permissive_autolinks();
//! ```

pub mod parser;
pub(crate) mod sys;
pub mod types;

#[cfg(feature = "html")]
pub mod html;

// Re-export main types at crate root
pub use parser::{parse, parse_to_events, Event, ParseError, ParseResult, ParserFlags, ParserHandler};
pub use types::{
    Alignment, Block, BlockType, CodeBlockDetail, FenceChar, HeadingDetail, ImageDetail,
    LinkDetail, ListItemDetail, ListMark, OrderedListDelimiter, OrderedListDetail, Span, SpanType,
    TableCellDetail, TableDetail, TaskState, TextType, UnorderedListDetail, WikiLinkDetail,
};

#[cfg(feature = "html")]
pub use html::{render_html, render_html_streaming, HtmlError, HtmlFlags, HtmlResult};

/// Convenience function to render markdown to HTML with default settings
///
/// Uses CommonMark dialect with default HTML renderer settings.
///
/// # Example
/// ```
/// let html = md4c::to_html("# Hello\n\nWorld").unwrap();
/// assert!(html.contains("<h1>Hello</h1>"));
/// ```
#[cfg(feature = "html")]
pub fn to_html(markdown: &str) -> HtmlResult<String> {
    render_html(
        markdown,
        ParserFlags::commonmark(),
        html::HtmlFlags::new(),
    )
}

/// Convenience function to render GitHub-flavored markdown to HTML
///
/// Uses GitHub dialect (tables, strikethrough, task lists, autolinks).
///
/// # Example
/// ```
/// let html = md4c::gfm_to_html("~~deleted~~").unwrap();
/// assert!(html.contains("<del>deleted</del>"));
/// ```
#[cfg(feature = "html")]
pub fn gfm_to_html(markdown: &str) -> HtmlResult<String> {
    render_html(markdown, ParserFlags::github(), html::HtmlFlags::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_html() {
        let html = to_html("# Test").unwrap();
        assert!(html.contains("<h1>Test</h1>"));
    }

    #[test]
    fn test_gfm_to_html() {
        let html = gfm_to_html("| A | B |\n|---|---|\n| 1 | 2 |").unwrap();
        assert!(html.contains("<table>"));
        assert!(html.contains("<td>1</td>"));
    }

    #[test]
    fn test_parser_handler() {
        struct Counter {
            blocks: usize,
            spans: usize,
            texts: usize,
        }

        impl ParserHandler for Counter {
            fn enter_block(&mut self, _: Block) -> bool {
                self.blocks += 1;
                true
            }
            fn enter_span(&mut self, _: Span) -> bool {
                self.spans += 1;
                true
            }
            fn text(&mut self, _: TextType, _: &str) -> bool {
                self.texts += 1;
                true
            }
        }

        let mut counter = Counter {
            blocks: 0,
            spans: 0,
            texts: 0,
        };
        parse("**bold** and *italic*", ParserFlags::commonmark(), &mut counter).unwrap();

        assert!(counter.blocks > 0);
        assert!(counter.spans > 0);
        assert!(counter.texts > 0);
    }

    #[test]
    fn test_parse_to_events() {
        let events = parse_to_events("Hello **world**", ParserFlags::commonmark()).unwrap();
        assert!(!events.is_empty());

        // Check for expected event types
        let has_strong = events
            .iter()
            .any(|e| matches!(e, parser::Event::EnterSpan(Span::Strong)));
        assert!(has_strong);
    }

    #[test]
    fn test_heading_levels() {
        struct HeadingChecker {
            levels: Vec<u8>,
        }

        impl ParserHandler for HeadingChecker {
            fn enter_block(&mut self, block: Block) -> bool {
                if let Block::Heading(detail) = block {
                    self.levels.push(detail.level);
                }
                true
            }
        }

        let mut checker = HeadingChecker { levels: vec![] };
        parse("# H1\n## H2\n### H3", ParserFlags::commonmark(), &mut checker).unwrap();
        assert_eq!(checker.levels, vec![1, 2, 3]);
    }

    #[test]
    fn test_link_details() {
        struct LinkChecker {
            href: String,
            title: String,
        }

        impl ParserHandler for LinkChecker {
            fn enter_span(&mut self, span: Span) -> bool {
                if let Span::Link(detail) = span {
                    self.href = detail.href;
                    self.title = detail.title;
                }
                true
            }
        }

        let mut checker = LinkChecker {
            href: String::new(),
            title: String::new(),
        };
        parse(
            r#"[text](https://example.com "Example")"#,
            ParserFlags::commonmark(),
            &mut checker,
        )
        .unwrap();

        assert_eq!(checker.href, "https://example.com");
        assert_eq!(checker.title, "Example");
    }

    #[test]
    fn test_code_block_info() {
        struct CodeChecker {
            lang: String,
        }

        impl ParserHandler for CodeChecker {
            fn enter_block(&mut self, block: Block) -> bool {
                if let Block::Code(detail) = block {
                    self.lang = detail.lang;
                }
                true
            }
        }

        let mut checker = CodeChecker { lang: String::new() };
        parse("```rust\nfn main() {}\n```", ParserFlags::commonmark(), &mut checker).unwrap();
        assert_eq!(checker.lang, "rust");
    }

    #[test]
    fn test_table_parsing() {
        struct TableChecker {
            col_count: u32,
        }

        impl ParserHandler for TableChecker {
            fn enter_block(&mut self, block: Block) -> bool {
                if let Block::Table(detail) = block {
                    self.col_count = detail.column_count;
                }
                true
            }
        }

        let mut checker = TableChecker { col_count: 0 };
        let table = "| A | B | C |\n|---|---|---|\n| 1 | 2 | 3 |";
        parse(table, ParserFlags::github(), &mut checker).unwrap();
        assert_eq!(checker.col_count, 3);
    }

    #[test]
    fn test_task_list() {
        struct TaskChecker {
            tasks: Vec<TaskState>,
        }

        impl ParserHandler for TaskChecker {
            fn enter_block(&mut self, block: Block) -> bool {
                if let Block::ListItem(detail) = block {
                    if detail.task_state != TaskState::NotTask {
                        self.tasks.push(detail.task_state);
                    }
                }
                true
            }
        }

        let mut checker = TaskChecker { tasks: vec![] };
        parse(
            "- [x] Done\n- [ ] Pending",
            ParserFlags::github(),
            &mut checker,
        )
        .unwrap();

        assert_eq!(checker.tasks.len(), 2);
        assert_eq!(checker.tasks[0], TaskState::Checked);
        assert_eq!(checker.tasks[1], TaskState::Unchecked);
    }
}

# ratatui-md

Markdown rendering for [ratatui](https://github.com/ratatui-org/ratatui) terminal UIs.

Uses [MD4C](https://github.com/mity/md4c) for fast, CommonMark-compliant parsing.

## Features

- **Full Markdown Support**: Headings, emphasis, links, code blocks, lists, blockquotes, tables
- **GitHub Flavored Markdown**: Tables, task lists, strikethrough, autolinks
- **Customizable Themes**: Built-in dark/light themes or create your own
- **Interactive Widgets**: Scrolling, link navigation, heading jumping
- **Syntax Highlighting**: Optional code block highlighting via syntect
- **Zero External Runtime Dependencies**: Pure Rust with embedded C parser

## Installation

```toml
[dependencies]
ratatui-md = { path = "path/to/ratatui-md" }

# Optional: syntax highlighting
ratatui-md = { path = "path/to/ratatui-md", features = ["syntect"] }
```

## Quick Start

```rust
use ratatui_md::Markdown;

let widget = Markdown::new("# Hello **World**");
// frame.render_widget(widget, area);
```

## Usage

### Basic Widget

```rust
use ratatui_md::{Markdown, Theme};
use ratatui::widgets::Block;

let markdown = Markdown::new(content)
    .theme(Theme::dark())
    .block(Block::default().title("Help"))
    .scroll((offset, 0));

frame.render_widget(markdown, area);
```

### Interactive Viewer

```rust
use ratatui_md::MarkdownView;

let mut view = MarkdownView::new(content);

// Scrolling
view.scroll_down(5);
view.scroll_up(2);
view.scroll_to_top();
view.scroll_to_bottom();

// Navigate headings
let headings = view.headings();
view.scroll_to_heading(0);

// Navigate links
view.select_next_link();
if let Some(link) = view.selected_link() {
    open_url(&link.url);
}

// Render
frame.render_widget(view.widget(), area);
```

### Direct Rendering

```rust
use ratatui_md::{render, Theme, RenderOptions};

let result = render(markdown, &Theme::default(), &RenderOptions::default());
let text = result.text;      // ratatui::text::Text
let links = result.links;    // Vec<LinkInfo>
let headings = result.headings;  // Vec<HeadingInfo>
```

### Custom Theme

```rust
use ratatui_md::Theme;
use ratatui::style::{Color, Modifier, Style};

let mut theme = Theme::default();
theme.heading1 = Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD);
theme.link = Style::new().fg(Color::Blue).add_modifier(Modifier::UNDERLINED);
theme.code_inline = Style::new().fg(Color::Yellow).bg(Color::DarkGray);
```

### Parser Options

```rust
use ratatui_md::{Markdown, RenderOptions};
use md4c::ParserFlags;

let options = RenderOptions::default()
    .with_parser_flags(
        ParserFlags::new()
            .tables()
            .strikethrough()
            .task_lists()
    );

let widget = Markdown::new(content).options(options);
```

## Themes

| Theme | Description |
|-------|-------------|
| `Theme::default()` | Balanced colors for most terminals |
| `Theme::dark()` | Optimized for dark backgrounds |
| `Theme::light()` | Optimized for light backgrounds |
| `Theme::plain()` | Minimal formatting, no colors |

## Markdown Support

| Element | Support | Notes |
|---------|---------|-------|
| Headings | ✓ | H1-H6 with distinct colors |
| Emphasis | ✓ | *italic*, **bold**, ***both*** |
| Strikethrough | ✓ | ~~text~~ (GFM) |
| Inline Code | ✓ | `code` |
| Code Blocks | ✓ | With language labels |
| Links | ✓ | With optional URL display |
| Images | ✓ | Rendered as [alt](src) |
| Lists | ✓ | Ordered, unordered, nested |
| Task Lists | ✓ | [x] checked, [ ] unchecked |
| Blockquotes | ✓ | With visual markers |
| Tables | ✓ | With borders and alignment |
| Horizontal Rules | ✓ | Full-width lines |
| LaTeX Math | ✓ | Rendered literally |
| Wiki Links | ✓ | [[link]] syntax |

## Examples

Run the demo:
```bash
cargo run --example demo
```

Run the interactive viewer:
```bash
cargo run --example scrollable
```

## Use Cases

- **Help Systems**: Display formatted help text in CLI apps
- **Documentation Viewers**: Browse markdown docs in the terminal
- **Chat Applications**: Render markdown messages
- **Note-Taking Apps**: Display formatted notes
- **README Viewers**: Preview markdown files
- **Config UIs**: Show formatted descriptions

## License

MIT License - same as MD4C and ratatui

# md4c-rs

Safe Rust bindings for [MD4C](https://github.com/mity/md4c), a fast CommonMark-compliant Markdown parser.

## Features

- **Fast**: MD4C is one of the fastest Markdown parsers available
- **CommonMark compliant**: Fully compliant with CommonMark 0.31
- **Zero-copy parsing**: Event-based API for efficient parsing
- **GitHub Flavored Markdown**: Support for GFM extensions:
  - Tables
  - Strikethrough (`~~text~~`)
  - Task lists (`- [x] done`)
  - Autolinks
  - Wiki links (`[[page]]`)
  - LaTeX math spans (`$x^2$`)
  - Underline

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
md4c-rs = "0.5"
```

## Usage

### Quick HTML Rendering

```rust
use md4c::{to_html, gfm_to_html};

// CommonMark
let html = to_html("# Hello **World**").unwrap();

// GitHub Flavored Markdown
let html = gfm_to_html("~~strikethrough~~").unwrap();
```

### Custom Parser Flags

```rust
use md4c::{render_html, ParserFlags, HtmlFlags};

let flags = ParserFlags::new()
    .tables()
    .strikethrough()
    .task_lists()
    .no_html();  // Disable HTML passthrough

let html = render_html(markdown, flags, HtmlFlags::new()).unwrap();
```

### Event-Based Parsing

For custom rendering or analysis:

```rust
use md4c::{parse, ParserFlags, ParserHandler, Block, Span, BlockType, SpanType, TextType};

struct MyHandler {
    depth: usize,
}

impl ParserHandler for MyHandler {
    fn enter_block(&mut self, block: Block) -> bool {
        self.depth += 1;
        println!("{:indent$}Enter: {:?}", "", block, indent = self.depth * 2);
        true
    }

    fn leave_block(&mut self, block_type: BlockType) -> bool {
        println!("{:indent$}Leave: {:?}", "", block_type, indent = self.depth * 2);
        self.depth -= 1;
        true
    }

    fn text(&mut self, text_type: TextType, text: &str) -> bool {
        println!("{:indent$}Text({:?}): {:?}", "", text_type, text, indent = self.depth * 2);
        true
    }
}

let mut handler = MyHandler { depth: 0 };
parse("# Title\n\nParagraph", ParserFlags::commonmark(), &mut handler).unwrap();
```

### Streaming HTML Output

For large documents:

```rust
use md4c::{render_html_streaming, ParserFlags, HtmlFlags};
use std::io::Write;

let mut file = std::fs::File::create("output.html").unwrap();
render_html_streaming(
    markdown,
    ParserFlags::github(),
    HtmlFlags::new(),
    |chunk| {
        file.write_all(chunk.as_bytes()).unwrap();
    }
).unwrap();
```

## Parser Flags

| Flag | Description |
|------|-------------|
| `commonmark()` | Standard CommonMark (no extensions) |
| `github()` | GitHub Flavored Markdown preset |
| `tables()` | Enable tables |
| `strikethrough()` | Enable `~~text~~` |
| `task_lists()` | Enable `- [x] task` |
| `wiki_links()` | Enable `[[links]]` |
| `latex_math_spans()` | Enable `$math$` and `$$display$$` |
| `underline()` | Enable `__underline__` |
| `no_html()` | Disable HTML passthrough |
| `permissive_autolinks()` | Auto-link URLs and emails |

## HTML Renderer Flags

| Flag | Description |
|------|-------------|
| `xhtml()` | Output XHTML (`<br />` instead of `<br>`) |
| `skip_utf8_bom()` | Skip UTF-8 BOM if present |
| `verbatim_entities()` | Don't decode HTML entities |

## Block Types

- `Document` - Root element
- `Paragraph` - Text paragraph
- `Heading(HeadingDetail)` - H1-H6 with level info
- `Code(CodeBlockDetail)` - Fenced or indented code with language
- `Quote` - Blockquote
- `UnorderedList(UnorderedListDetail)` - Bullet list with marker info
- `OrderedList(OrderedListDetail)` - Numbered list with start number
- `ListItem(ListItemDetail)` - List item with task state
- `Table(TableDetail)` - Table with column count
- `HorizontalRule` - `---` or `***`
- `Html` - Raw HTML block

## Span Types

- `Emphasis` - `*italic*`
- `Strong` - `**bold**`
- `Code` - `` `code` ``
- `Link(LinkDetail)` - `[text](url)` with href/title
- `Image(ImageDetail)` - `![alt](src)` with src/title
- `Strikethrough` - `~~deleted~~`
- `WikiLink(WikiLinkDetail)` - `[[page]]`
- `LatexMath` / `LatexMathDisplay` - `$x$` / `$$x$$`
- `Underline` - `__underlined__`

## License

MIT License - same as MD4C

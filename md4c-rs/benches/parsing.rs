use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use md4c::{gfm_to_html, parse, to_html, ParserFlags, ParserHandler};

const SIMPLE_MD: &str = r#"
# Hello World

This is a simple paragraph with some **bold** and *italic* text.

- Item 1
- Item 2
- Item 3

> A blockquote with some content.

```rust
fn main() {
    println!("Hello!");
}
```
"#;

const COMPLEX_MD: &str = r#"
# Complex Document

This document tests various Markdown features including [links](https://example.com "Example"),
**bold text**, *italic text*, and `inline code`.

## Tables

| Column A | Column B | Column C |
|:---------|:--------:|---------:|
| Left     | Center   | Right    |
| Data 1   | Data 2   | Data 3   |
| More     | Data     | Here     |

## Lists

1. First ordered item
2. Second ordered item
   - Nested unordered
   - Another nested
3. Third ordered item

### Task Lists

- [x] Completed task
- [ ] Pending task
- [x] Another done

## Code Blocks

```python
def hello():
    """A simple function."""
    return "Hello, World!"

if __name__ == "__main__":
    print(hello())
```

## Blockquotes

> This is a blockquote.
> It spans multiple lines.
>
> > Nested blockquote here.

## Emphasis Combinations

This has ***bold and italic*** text, plus ~~strikethrough~~.

## Links and Images

Check out [this link](https://example.com) and ![an image](https://example.com/image.png).

---

The end.
"#;

struct NullHandler;

impl ParserHandler for NullHandler {
    fn enter_block(&mut self, _: md4c::Block) -> bool {
        true
    }
    fn leave_block(&mut self, _: md4c::BlockType) -> bool {
        true
    }
    fn enter_span(&mut self, _: md4c::Span) -> bool {
        true
    }
    fn leave_span(&mut self, _: md4c::SpanType) -> bool {
        true
    }
    fn text(&mut self, _: md4c::TextType, _: &str) -> bool {
        true
    }
}

fn bench_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parsing");

    group.throughput(Throughput::Bytes(SIMPLE_MD.len() as u64));
    group.bench_function("simple_parse", |b| {
        b.iter(|| {
            let mut handler = NullHandler;
            parse(black_box(SIMPLE_MD), ParserFlags::commonmark(), &mut handler).unwrap();
        });
    });

    group.throughput(Throughput::Bytes(COMPLEX_MD.len() as u64));
    group.bench_function("complex_parse_commonmark", |b| {
        b.iter(|| {
            let mut handler = NullHandler;
            parse(black_box(COMPLEX_MD), ParserFlags::commonmark(), &mut handler).unwrap();
        });
    });

    group.bench_function("complex_parse_github", |b| {
        b.iter(|| {
            let mut handler = NullHandler;
            parse(black_box(COMPLEX_MD), ParserFlags::github(), &mut handler).unwrap();
        });
    });

    group.finish();
}

fn bench_html_rendering(c: &mut Criterion) {
    let mut group = c.benchmark_group("html_rendering");

    group.throughput(Throughput::Bytes(SIMPLE_MD.len() as u64));
    group.bench_function("simple_to_html", |b| {
        b.iter(|| {
            to_html(black_box(SIMPLE_MD)).unwrap();
        });
    });

    group.throughput(Throughput::Bytes(COMPLEX_MD.len() as u64));
    group.bench_function("complex_to_html", |b| {
        b.iter(|| {
            to_html(black_box(COMPLEX_MD)).unwrap();
        });
    });

    group.bench_function("complex_gfm_to_html", |b| {
        b.iter(|| {
            gfm_to_html(black_box(COMPLEX_MD)).unwrap();
        });
    });

    group.finish();
}

criterion_group!(benches, bench_parsing, bench_html_rendering);
criterion_main!(benches);

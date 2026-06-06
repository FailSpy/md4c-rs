//! Verify the Step 2 `TextContext::source_offset` contract:
//!
//! **Load-bearing invariant** (the one downstream code relies on):
//! when `source_offset == Some(o)`, slicing `input[o..o + text.len()]`
//! must produce *exactly* the bytes the handler received as `text`.
//! This is the safety property — a Some offset must always round-trip.
//!
//! Consequence-of-MD4C-behavior (NOT a Cadenza-side invariant): MD4C
//! delivers entity source text (`&amp;`) with `TextType::Entity` from
//! the input buffer, not the decoded glyph from a scratch buffer.
//! Consumers that need the decoded form must call MD4C's decoder. The
//! `source_offset` for entities is therefore typically `Some`.
//!
//! Where `None` can appear: text MD4C synthesizes outside the input
//! buffer — some line-break / NUL-character / whitespace-collapsing
//! cases. The Cadenza-side renderer's delimiter-walk fallback handles
//! those whenever they arise.
//!
//! No UB: even on adversarial inputs (entity-heavy, multi-byte UTF-8,
//! large inputs), the parse completes without panic or undefined
//! behavior. The pointer arithmetic in `text_cb` uses pure `usize`
//! with both-way bounds checks and `is_char_boundary` validation.

use md4c::{parse, Event, ParseError, ParserFlags, TextContext, TextType};

/// Helper: collect all text events with their `(text_type, text, source_offset)`.
fn collect_texts(input: &str, flags: ParserFlags) -> Vec<(TextType, String, Option<u32>)> {
    let events = md4c::parse_to_events(input, flags).expect("parse");
    events
        .into_iter()
        .filter_map(|ev| match ev {
            Event::Text(tt, s, ctx) => Some((tt, s, ctx.source_offset)),
            _ => None,
        })
        .collect()
}

#[test]
fn normal_text_carries_source_offset_that_indexes_back_into_input() {
    let input = "Hello, **world**!";
    let texts = collect_texts(input, ParserFlags::commonmark());

    for (tt, text, off) in &texts {
        if let (TextType::Normal, Some(o)) = (tt, off) {
            let start = *o as usize;
            let end = start + text.len();
            assert!(end <= input.len(), "offset+len out of range for {:?}", text);
            assert_eq!(&input[start..end], text.as_str(), "round-trip slice");
        }
    }

    assert!(
        texts.iter().any(|(_, _, off)| off.is_some()),
        "expected at least one text run with Some(offset); got {:?}",
        texts
    );
}

#[test]
fn all_some_offsets_round_trip_byte_exact() {
    // The load-bearing invariant for every test corpus item:
    // any text whose source_offset is Some MUST round-trip exactly.
    // This is the safety contract downstream MarkdownSourceMap will
    // rely on for source-mode copy.
    let inputs = &[
        "plain prose",
        "Hello, **world**!",
        "Hello &amp; goodbye",
        "Foo &amp; bar &lt; baz &gt; qux",
        "# Heading\n\nParagraph with `code` and `**emphasis**`.",
        "pre &amp; post",
        "Héllo, 日本語 world",
        "- item one\n- item two\n- item three\n",
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
        "```rust\nfn main() {}\n```\n",
        "[link](https://example.com)",
        "![image](path.png)",
    ];

    for input in inputs {
        let texts = collect_texts(input, ParserFlags::github());
        for (_tt, text, off) in &texts {
            if let Some(o) = off {
                let start = *o as usize;
                let end = start + text.len();
                assert!(
                    end <= input.len(),
                    "input {:?}: offset {} + len {} > input.len() {} for text {:?}",
                    input,
                    start,
                    text.len(),
                    input.len(),
                    text,
                );
                assert_eq!(
                    &input[start..end],
                    text.as_str(),
                    "input {:?}: text {:?} at offset {} does not round-trip; got {:?}",
                    input,
                    text,
                    start,
                    &input[start..end],
                );
            }
        }
    }
}

#[test]
fn entity_source_text_is_in_input_buffer() {
    // Per MD4C's actual behavior: entity source text (e.g., the literal
    // `&amp;`) lives in the input buffer. The handler receives `&amp;`
    // (NOT the decoded `&`) with TextType::Entity as a hint. The
    // source_offset therefore round-trips like Normal text.
    let input = "before &amp; after";
    let texts = collect_texts(input, ParserFlags::commonmark());

    let entity = texts
        .iter()
        .find(|(tt, _, _)| matches!(tt, TextType::Entity))
        .expect("at least one Entity text expected");

    let (_, text, off) = entity;
    assert_eq!(text, "&amp;", "MD4C delivers the literal entity source");

    if let Some(o) = off {
        let start = *o as usize;
        let end = start + text.len();
        assert_eq!(&input[start..end], text);
    }
    // If MD4C ever delivered the entity from a scratch buffer in some
    // build configuration, source_offset would be None and the Some-side
    // round-trip wouldn't fire — both outcomes are spec-compatible.
    // What's NOT spec-compatible is `Some(o)` that doesn't round-trip;
    // the assertion above covers that.
}

#[test]
fn empty_input_does_not_panic() {
    // Adversarial: empty input. input_end = input_start + 0 = input_start;
    // no text events fire; no UB.
    let texts = collect_texts("", ParserFlags::commonmark());
    assert_eq!(texts.len(), 0);
}

#[test]
fn unicode_text_offsets_are_at_char_boundaries() {
    // Multi-byte UTF-8: offsets must land on char boundaries (verified
    // by `is_char_boundary` in text_cb). Slicing must not panic.
    let input = "Héllo, 日本語 world";
    let texts = collect_texts(input, ParserFlags::commonmark());

    for (_, text, off) in &texts {
        if let Some(o) = off {
            let start = *o as usize;
            let end = start + text.len();
            assert!(
                input.is_char_boundary(start),
                "offset {} not at char boundary in {:?}",
                start,
                input
            );
            assert!(
                input.is_char_boundary(end),
                "end {} not at char boundary in {:?}",
                end,
                input
            );
            assert_eq!(&input[start..end], text.as_str());
        }
    }
}

#[test]
fn very_large_input_no_overflow_no_panic() {
    // Pathological-ish: ~64 KB input. checked_add prevents usize
    // overflow on the bounds check; this exercises the path. The
    // load-bearing claim: round-trip holds for all Some offsets.
    let mut input = String::with_capacity(64 * 1024);
    for i in 0..1024 {
        input.push_str(&format!("para {} with &amp; entity\n\n", i));
    }
    let texts = collect_texts(&input, ParserFlags::commonmark());
    assert!(
        !texts.is_empty(),
        "expected text events for non-empty input"
    );

    for (_, text, off) in &texts {
        if let Some(o) = off {
            let start = *o as usize;
            let end = start + text.len();
            assert!(end <= input.len(), "overflow in large input");
            assert_eq!(
                &input[start..end],
                text.as_str(),
                "round-trip in large input"
            );
        }
    }
}

#[test]
fn parse_returns_ok_on_normal_input() {
    // Sanity: the new code path doesn't break successful parse outcomes.
    let result: Result<(), ParseError> = parse(
        "# Heading\n\nA paragraph with `code` and **bold**.",
        ParserFlags::commonmark(),
        &mut NullHandler,
    );
    assert!(result.is_ok());
}

#[test]
fn nul_replacement_text_may_have_none_offset() {
    // NUL bytes in input are replaced with U+FFFD by MD4C, which often
    // comes from a scratch buffer. This is one path where source_offset
    // legitimately becomes None. We don't *require* None here (depends on
    // MD4C build configuration); we DO require the safety invariant: if
    // Some, it round-trips.
    let mut input = String::from("before ");
    input.push('\u{0000}');
    input.push_str(" after");

    let texts = collect_texts(&input, ParserFlags::commonmark());
    for (_, text, off) in &texts {
        if let Some(o) = off {
            let start = *o as usize;
            let end = start + text.len();
            assert!(end <= input.len());
            assert_eq!(&input[start..end], text.as_str());
        }
        // If off is None, that's the documented scratch-buffer case —
        // downstream code must handle it via delimiter-walk fallback.
    }
}

struct NullHandler;
impl md4c::ParserHandler for NullHandler {
    fn text(&mut self, _: TextType, _: &str, _: TextContext) -> bool {
        true
    }
}

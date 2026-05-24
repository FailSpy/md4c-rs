//! Step 3b quality gate (syntect half): the `highlight_with_offsets` API
//! returns byte-offset side-channel data whose offsets round-trip
//! byte-exactly into the original code string.
//!
//! Skipped when the `syntect` feature is off: the no-syntect placeholder
//! returns a single-span-per-line shape that's trivially correct.

#![cfg(feature = "syntect")]

use ratatui_md::SyntaxHighlighter;

#[test]
fn highlight_with_offsets_round_trips_per_span() {
    let highlighter = SyntaxHighlighter::new();
    let code = "fn main() {\n    println!(\"hi\");\n    let x = 42;\n}\n";

    let lines = highlighter.highlight_with_offsets(code, "rust");
    assert!(!lines.is_empty(), "syntect produced no lines");

    for (line_idx, line) in lines.iter().enumerate() {
        assert_eq!(
            line.spans.len(),
            line.span_offsets.len(),
            "line {}: spans/offsets length mismatch",
            line_idx
        );

        for (span_idx, (span, offset)) in
            line.spans.iter().zip(line.span_offsets.iter()).enumerate()
        {
            let Some((start, end)) = offset else { continue };
            let s = *start as usize;
            let e = *end as usize;
            assert!(
                e <= code.len(),
                "line {} span {}: end {} > code.len() {}",
                line_idx, span_idx, e, code.len()
            );
            assert!(
                code.is_char_boundary(s) && code.is_char_boundary(e),
                "line {} span {}: offsets {:?} not on char boundaries",
                line_idx, span_idx, offset
            );

            // The strong round-trip claim:
            //   code[start..end] == span.content
            assert_eq!(
                &code[s..e],
                span.content.as_ref(),
                "line {} span {}: source slice does not byte-equal span content; \
                 offset={:?}, source={:?}, span={:?}",
                line_idx, span_idx, offset, &code[s..e], span.content
            );
        }
    }
}

#[test]
fn line_byte_starts_are_monotonic_and_in_range() {
    let highlighter = SyntaxHighlighter::new();
    let code = "fn one() {}\nfn two() {}\nfn three() {}\n";
    let lines = highlighter.highlight_with_offsets(code, "rust");

    let mut prev: u32 = 0;
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            assert!(
                line.line_byte_start > prev,
                "line_byte_start not monotonic at line {}",
                i
            );
        }
        assert!(
            (line.line_byte_start as usize) <= code.len(),
            "line {} byte_start {} out of range",
            i, line.line_byte_start
        );
        prev = line.line_byte_start;
    }
}

#[test]
fn empty_code_does_not_panic() {
    let highlighter = SyntaxHighlighter::new();
    let _ = highlighter.highlight_with_offsets("", "rust");
}

#[test]
fn unicode_in_code_block_round_trips() {
    let highlighter = SyntaxHighlighter::new();
    let code = "// café 日本語\nfn naïve() {}\n";
    let lines = highlighter.highlight_with_offsets(code, "rust");

    for line in &lines {
        for (span, offset) in line.spans.iter().zip(line.span_offsets.iter()) {
            let Some((s, e)) = offset else { continue };
            assert_eq!(
                &code[*s as usize..*e as usize],
                span.content.as_ref(),
                "unicode round-trip fail"
            );
        }
    }
}

#[test]
fn plain_text_falls_back_with_offsets() {
    // Unknown language: syntect falls back to plain-text syntax. The
    // offsets should still round-trip.
    let highlighter = SyntaxHighlighter::new();
    let code = "hello\nworld\n";
    let lines = highlighter.highlight_with_offsets(code, "nonexistent-lang");
    assert!(!lines.is_empty());
    for line in &lines {
        for (span, offset) in line.spans.iter().zip(line.span_offsets.iter()) {
            let Some((s, e)) = offset else { continue };
            assert_eq!(&code[*s as usize..*e as usize], span.content.as_ref());
        }
    }
}

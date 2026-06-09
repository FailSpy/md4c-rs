//! Step 3a quality gate: round-trip property test.
//!
//! For every content grapheme in a rendered markdown block, the
//! `MarkdownSourceMap`'s `anchor_to_source` MUST return a byte span that
//! indexes correctly into the projected source (which, in Step 3a's
//! pre-privacy world, is the input markdown verbatim). Specifically:
//!
//! - `&source[span.start..span.end]` must be a valid UTF-8 slice on a
//!   grapheme boundary.
//! - For text runs that MD4C delivered from the input buffer (i.e.,
//!   `TextContext::source_offset.is_some()`), the slice must round-trip
//!   to the exact bytes the renderer rendered for that grapheme.
//!
//! Covered constructs (12 common):
//!  1. Plain prose
//!  2. Bold (`**text**`)
//!  3. Italic (`*text*` / `_text_`)
//!  4. Inline code (`` `code` ``)
//!  5. Strikethrough (`~~text~~`)
//!  6. Link text (`[text](url)`)
//!  7. Heading content
//!  8. Unordered list item content
//!  9. Ordered list item content
//! 10. Blockquote content
//! 11. Fenced code block content
//! 12. Mixed nested formatting (`**bold _and italic_**`)

use cadenza_anchor::{Anchor, BlockId, DecorativeKind, SourceMapping};
use ratatui_md::{render_with_block, RenderOptions, Theme};

const BLOCK: BlockId = BlockId(7);

/// Walk every content grapheme in the rendered output, look up its source
/// span via `anchor_to_source`, and BYTE-EXACT compare:
///   `&source[span.start..span.end]` MUST equal the bytes of the rendered
///   grapheme (extracted from `rendered.text` at the matching position).
///
/// Returns `(round_tripped_count, mapped_count, content_count)`.
/// - `content_count`: total content graphemes (decoratives excluded).
/// - `mapped_count`: of those, how many had `Some(span)` (the rest are
///   scratch-buffer graphemes, normalized whitespace, etc. — None per the
///   delimiter-walk-fallback policy).
/// - `round_tripped_count`: of the mapped ones, how many had source bytes
///   that byte-equaled the rendered grapheme. MUST equal `mapped_count`
///   for Step 3a's gate to pass.
fn round_trip_content_graphemes(input: &str) -> (usize, usize, usize) {
    use unicode_segmentation::UnicodeSegmentation;

    let opts = RenderOptions::github();
    let (rendered, source_map) = render_with_block(input, &Theme::default(), &opts, BLOCK);

    // For each logical line in the source map, find the matching rendered
    // line and extract its grapheme sequence. The position_map is built per
    // input line in the renderer; the rendered Text's Line slots correspond
    // 1:1 with position_map's LinePosMap slots (modulo trailing empties).
    let rendered_lines: Vec<String> = rendered
        .text
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect();

    let mut round_tripped = 0usize;
    let mut mapped = 0usize;
    let mut content = 0usize;

    for (line_idx, logical_line) in source_map.logical_lines().iter().enumerate() {
        // Collect the rendered graphemes on this line, in order.
        let rendered_line = rendered_lines
            .get(line_idx)
            .map(String::as_str)
            .unwrap_or("");
        let rendered_graphemes: Vec<&str> = rendered_line.graphemes(true).collect();

        for (within_line_idx, grapheme_info) in logical_line.graphemes.iter().enumerate() {
            // Skip decoratives — round-trip contract doesn't apply to cells
            // without source backing.
            if grapheme_info.decorative.is_some() {
                continue;
            }
            content += 1;

            // Flat grapheme index used by Anchor: sum prior lines + within-line offset.
            let mut flat: u32 = 0;
            for prev_line in source_map.logical_lines().iter().take(line_idx) {
                flat += prev_line.graphemes.len() as u32;
            }
            flat += within_line_idx as u32;

            let anchor = Anchor {
                block: BLOCK,
                grapheme: flat,
            };

            let Some(span) = source_map.anchor_to_source(anchor) else {
                // Scratch-buffer grapheme — not mapped, not counted as failure.
                continue;
            };
            mapped += 1;

            // Strong oracle: the sliced source bytes MUST byte-equal the
            // rendered grapheme. This catches stale-offset bugs (e.g., a
            // synthetic `" "` that's incorrectly mapped to a `\n` source
            // byte) which a UTF-8-validity-only check would miss.
            let source = source_map.source();
            let source_end = span.end as usize;
            let source_start = span.start as usize;
            assert!(
                source_end <= source.len(),
                "input {:?}: span {:?} exceeds source.len()={} (line {} grapheme {})",
                input,
                span,
                source.len(),
                line_idx,
                within_line_idx
            );
            let source_slice = &source.as_bytes()[source_start..source_end];

            let rendered_grapheme = rendered_graphemes
                .get(within_line_idx)
                .copied()
                .unwrap_or("");

            assert_eq!(
                source_slice,
                rendered_grapheme.as_bytes(),
                "input {:?}: round-trip mismatch at line {} grapheme {} \
                 (flat {}): source[{}..{}]={:?} != rendered={:?}",
                input,
                line_idx,
                within_line_idx,
                flat,
                source_start,
                source_end,
                std::str::from_utf8(source_slice).unwrap_or("<bad utf8>"),
                rendered_grapheme,
            );
            round_tripped += 1;
        }
    }

    (round_tripped, mapped, content)
}

/// All-or-nothing assertion: every mapped content grapheme MUST byte-equal
/// its source slice. The inner per-grapheme assertion in
/// `round_trip_content_graphemes` is the real oracle; the outer counts
/// here exist to ensure the test exercises a non-trivial number of cells.
fn assert_strong_round_trip(input: &str) {
    let (round_tripped, mapped, content) = round_trip_content_graphemes(input);
    assert!(content > 0, "input {:?}: expected content graphemes", input);
    assert!(
        mapped > 0,
        "input {:?}: expected at least one mapped grapheme; got {}/{}/{}",
        input,
        round_tripped,
        mapped,
        content
    );
    assert_eq!(
        round_tripped, mapped,
        "input {:?}: every mapped grapheme must round-trip byte-exactly; \
         got {}/{} round-tripped (content total: {})",
        input, round_tripped, mapped, content
    );
}

#[test]
fn plain_prose_round_trips() {
    assert_strong_round_trip("Hello world, this is plain prose.");
}

#[test]
fn bold_round_trips() {
    assert_strong_round_trip("Some **bold text** in a paragraph.");
}

#[test]
fn italic_round_trips() {
    for input in ["*italic*", "_italic_", "Some *italic words* mid sentence."] {
        assert_strong_round_trip(input);
    }
}

#[test]
fn inline_code_round_trips() {
    assert_strong_round_trip("Use the `println!` macro for output.");
}

#[test]
fn strikethrough_round_trips() {
    assert_strong_round_trip("Old idea: ~~the moon is made of cheese~~ — debunked.");
}

#[test]
fn link_text_round_trips() {
    assert_strong_round_trip("See [the docs](https://example.com/docs) for details.");
}

#[test]
fn heading_round_trips() {
    for input in ["# Heading 1", "## Subheading", "### Three deep"] {
        assert_strong_round_trip(input);
    }
}

#[test]
fn unordered_list_round_trips() {
    assert_strong_round_trip("- first item\n- second item\n- third item\n");
}

#[test]
fn ordered_list_round_trips() {
    assert_strong_round_trip("1. first\n2. second\n3. third\n");
}

#[test]
fn blockquote_round_trips() {
    // Blockquote bar tagging is deferred to 3b (see source_map.rs module
    // header). Until then, position_map lines for blockquoted content are
    // internally consistent but skip the `▎ ` bar — so the round-trip via
    // flat grapheme index still passes for the content itself.
    assert_strong_round_trip("> A quoted line of text\n> with a continuation\n");
}

#[test]
fn fenced_code_block_round_trips() {
    // render_with_block force-disables syntax_highlighting (deferred to
    // 3b); the code-block content is rendered as plain text through
    // push_text, so position mappings are emitted and round-trip works.
    assert_strong_round_trip("```rust\nfn main() { println!(\"hi\"); }\n```\n");
}

#[test]
fn nested_formatting_round_trips() {
    assert_strong_round_trip("Try **bold _and italic_** together.");
}

#[test]
fn block_id_identity_gate_enforced() {
    // Build a source map with a known block id; query with a different id
    // → all anchor_to_* must return None (identity gate).
    let input = "Some plain text.";
    let opts = RenderOptions::github();
    let (_rendered, source_map) = render_with_block(input, &Theme::default(), &opts, BlockId(11));

    let wrong = Anchor {
        block: BlockId(99),
        grapheme: 0,
    };
    assert_eq!(source_map.anchor_to_source(wrong), None);
    assert_eq!(source_map.anchor_to_source_kind(wrong), None);
    assert_eq!(source_map.anchor_to_decorative(wrong), None);

    // Same id, in-range anchor → at least one of the three returns Some.
    let right = Anchor {
        block: BlockId(11),
        grapheme: 0,
    };
    let any = source_map.anchor_to_source(right).is_some()
        || source_map.anchor_to_source_kind(right).is_some()
        || source_map.anchor_to_decorative(right).is_some();
    assert!(
        any,
        "matching block must yield Some from at least one accessor"
    );
}

#[test]
fn unicode_content_round_trips() {
    // Multi-byte UTF-8 + combining marks. The per-grapheme span
    // computation uses `grapheme_indices`, so byte offsets land on
    // cluster boundaries. The strong oracle in
    // round_trip_content_graphemes now asserts byte-equality between
    // sliced source and rendered grapheme, including multi-byte sequences.
    assert_strong_round_trip("café 日本語 — naïve, résumé.");
}

#[test]
fn list_bullets_are_tagged_decorative() {
    // The first grapheme of a list item line should be a decorative
    // ListBullet (the "- " or "1. " prefix) per Step 3a's decorative
    // tagging coverage.
    let input = "- alpha\n- beta\n";
    let opts = RenderOptions::github();
    let (_rendered, source_map) = render_with_block(input, &Theme::default(), &opts, BlockId(7));

    let mut saw_bullet = false;
    for line in source_map.logical_lines() {
        if let Some(first) = line.graphemes.first() {
            if first.decorative == Some(DecorativeKind::ListBullet) {
                saw_bullet = true;
                // Decorative exclusivity: source MUST be None for decoratives.
                assert!(
                    first.source.is_none(),
                    "decorative cell unexpectedly carried a source span"
                );
                break;
            }
        }
    }
    assert!(
        saw_bullet,
        "expected at least one ListBullet decorative cell"
    );
}

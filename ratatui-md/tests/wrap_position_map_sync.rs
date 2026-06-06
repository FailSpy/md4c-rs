//! Step 3d gate: `position_map.line_count()` MUST equal
//! `rendered.text.lines.len()` for any (input, width) combination.
//!
//! Before Step 3d, the wrap path collapsed many visual lines into a single
//! position_map line — every grapheme of a wrapped paragraph shared one
//! line entry. That made flat-index grapheme lookups (the only
//! grapheme-addressing API source-mode copy walks) silently drift across
//! wrap boundaries. Step 3d's contract: post-wrap, the position_map's
//! shape mirrors the rendered Text's shape exactly.
//!
//! The tests below cover:
//!   - paragraphs that wrap on whitespace (no decoratives)
//!   - blockquotes that wrap (decorative prefix per visual line)
//!   - list items that wrap (hanging-indent on continuations)
//!   - nested constructs (list with inline emphasis that wraps)
//!   - long unbreakable tokens that hard-break at the budget

use cadenza_anchor::BlockId;
use ratatui_md::{render_with_block, RenderOptions, Theme};

const BLOCK: BlockId = BlockId(31);

fn render_at(input: &str, width: usize) -> (usize, usize) {
    let opts = RenderOptions::github().with_width(width);
    let (rendered, _) = render_with_block(input, &Theme::default(), &opts, BLOCK);
    let line_count = rendered.text.lines.len();
    let pm_line_count = rendered
        .position_map
        .as_ref()
        .map(|pm| pm.line_count())
        .expect("render_with_block forces track_positions on");
    (line_count, pm_line_count)
}

#[test]
fn paragraph_wrap_line_counts_match() {
    let input = "This is a long sentence that will definitely wrap at narrow widths because it has many words.";
    for &w in &[20usize, 30, 40, 60] {
        let (text_lines, pm_lines) = render_at(input, w);
        assert_eq!(
            text_lines, pm_lines,
            "wrap mismatch at width={w}: text={text_lines} pm={pm_lines}"
        );
    }
}

#[test]
fn blockquote_wrap_line_counts_match() {
    let input =
        "> This is a long blockquote that will wrap at narrow widths and each visual row should have its own position_map line.";
    for &w in &[20usize, 30, 50] {
        let (text_lines, pm_lines) = render_at(input, w);
        assert_eq!(
            text_lines, pm_lines,
            "blockquote wrap mismatch at width={w}: text={text_lines} pm={pm_lines}"
        );
    }
}

#[test]
fn list_item_wrap_line_counts_match() {
    let input = "- This list item is sufficiently long that it definitely wraps at narrow terminal widths\n- And here's a second short one";
    for &w in &[20usize, 30, 50] {
        let (text_lines, pm_lines) = render_at(input, w);
        assert_eq!(
            text_lines, pm_lines,
            "list wrap mismatch at width={w}: text={text_lines} pm={pm_lines}"
        );
    }
}

#[test]
fn nested_emphasis_wrap_line_counts_match() {
    let input = "Here is **a bold span** and *some italic* and `inline code` mixed in with plain prose that wraps.";
    for &w in &[20usize, 30, 40] {
        let (text_lines, pm_lines) = render_at(input, w);
        assert_eq!(
            text_lines, pm_lines,
            "nested emphasis wrap mismatch at width={w}: text={text_lines} pm={pm_lines}"
        );
    }
}

#[test]
fn unbreakable_token_wrap_line_counts_match() {
    // No whitespace anywhere — the wrap engine should hard-break at the
    // budget. Currently `wrap_spans_to_width` overflows (the documented
    // "single overlong words still overflow" path); Step 3d makes the
    // line count predictable by hard-breaking.
    let input = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    for &w in &[10usize, 20] {
        let (text_lines, pm_lines) = render_at(input, w);
        assert_eq!(
            text_lines, pm_lines,
            "unbreakable wrap mismatch at width={w}: text={text_lines} pm={pm_lines}"
        );
    }
}

/// Step 3d hanging-indent invariant: continuation visual lines of a
/// wrapped list item carry `HardWrapIndent` decoratives matching the
/// bullet's display width. (Without this, continuation rows flow flush
/// to column 0 — the visible bug Cadenza's second-pass wrap fixed.)
#[test]
fn list_continuation_lines_have_hard_wrap_indent_decoratives() {
    use cadenza_anchor::DecorativeKind;
    let input =
        "- This list item is sufficiently long that it definitely wraps at a narrow terminal width";
    let opts = ratatui_md::RenderOptions::github().with_width(30);
    let (rendered, _) =
        ratatui_md::render_with_block(input, &ratatui_md::Theme::default(), &opts, BLOCK);
    let pm = rendered.position_map.expect("track_positions on");
    assert!(
        rendered.text.lines.len() >= 3,
        "expected wrap to multiple rows at width=30"
    );

    // First line: starts with ListBullet decoratives ("• " or similar).
    let line0 = pm.line(0).expect("line 0");
    let leading_bullets = line0
        .iter()
        .take_while(|m| matches!(m.decorative, Some(DecorativeKind::ListBullet)))
        .count();
    assert!(
        leading_bullets > 0,
        "line 0 missing leading ListBullet decoratives"
    );

    // Continuation lines: start with HardWrapIndent decoratives whose
    // count matches the bullet width.
    for k in 1..rendered.text.lines.len() {
        let line = pm.line(k).expect("continuation line");
        if line.is_empty() {
            continue;
        }
        let leading_hard_indent = line
            .iter()
            .take_while(|m| matches!(m.decorative, Some(DecorativeKind::HardWrapIndent)))
            .count();
        assert_eq!(
            leading_hard_indent, leading_bullets,
            "line {} hanging-indent width mismatch: got {} HardWrapIndent cells, \
             want {} (matching bullet width)",
            k, leading_hard_indent, leading_bullets
        );
    }
}

/// Blockquote wrap repeats the bar on every visual line: each line's
/// position_map starts with `BlockquoteBar` decoratives matching the
/// rendered bar's grapheme count.
#[test]
fn blockquote_continuation_lines_repeat_bar_decoratives() {
    use cadenza_anchor::DecorativeKind;
    let input =
        "> This is a long blockquote that definitely wraps at narrow widths and continuation rows must repeat the bar";
    let opts = ratatui_md::RenderOptions::github().with_width(30);
    let (rendered, _) =
        ratatui_md::render_with_block(input, &ratatui_md::Theme::default(), &opts, BLOCK);
    let pm = rendered.position_map.expect("track_positions on");
    assert!(
        rendered.text.lines.len() >= 3,
        "expected blockquote wrap to multiple rows"
    );

    for k in 0..rendered.text.lines.len() {
        let line = pm.line(k).expect("line");
        if line.is_empty() {
            continue;
        }
        let leading_bars = line
            .iter()
            .take_while(|m| matches!(m.decorative, Some(DecorativeKind::BlockquoteBar)))
            .count();
        assert!(
            leading_bars > 0,
            "blockquote line {} missing leading BlockquoteBar decoratives",
            k
        );
    }
}

/// CJK content wraps cleanly at ideograph boundaries with synchronized
/// position_map. Each ideograph cluster is 2 cells wide; rendered Text
/// lines must each have a matching position_map line.
#[test]
fn cjk_wrap_line_counts_match() {
    // 12 CJK ideographs (24 cells total) at width=10 → ≥ 3 visual lines.
    let input = "一二三四五六七八九十一二";
    for &w in &[10usize, 14, 20] {
        let (text_lines, pm_lines) = render_at(input, w);
        assert_eq!(
            text_lines, pm_lines,
            "CJK wrap mismatch at width={w}: text={text_lines} pm={pm_lines}"
        );
    }
}

/// ZWJ family emoji stays as one cluster across wrap. The wrap engine
/// MUST treat the whole ZWJ sequence as a single grapheme cluster and
/// place it intact on one visual line; position_map gets exactly one
/// `CharMapping` for it.
#[test]
fn zwj_emoji_wrap_line_counts_match() {
    // U+1F468 U+200D U+1F469 U+200D U+1F466 — "man + woman + boy" family emoji.
    let input = "Hello \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F466} family";
    for &w in &[10usize, 14, 20] {
        let (text_lines, pm_lines) = render_at(input, w);
        assert_eq!(
            text_lines, pm_lines,
            "ZWJ emoji wrap mismatch at width={w}: text={text_lines} pm={pm_lines}"
        );
    }
}

/// Regression for Claude-F1 (HIGH): the soft-wrap-and-retry branch in
/// `wrap_clusters` looped forever for list items at narrow widths
/// when the continuation indent left no room for the next word. The
/// fix gates the soft-wrap branch on `word_width <= cont_body_budget`
/// so a word that can't fit even on a fresh continuation hard-breaks
/// instead. Trigger window: `effective_width - prefix_cells -
/// cont_indent_cells - word_width < 0` AND `cont_indent_cells > 0`.
#[test]
fn list_item_at_narrow_widths_does_not_hang() {
    let cases: &[(&str, usize)] = &[
        ("- One two\n", 6),
        ("- aa bb cc\n", 5),
        ("- The quick brown fox jumps", 8),
        ("- supercalifragilistic\n", 10),
        ("> One two three four\n", 5),
        ("1. First second third\n", 7),
    ];
    for (input, width) in cases {
        let opts = ratatui_md::RenderOptions::github().with_width(*width);
        let (rendered, _) =
            ratatui_md::render_with_block(input, &ratatui_md::Theme::default(), &opts, BLOCK);
        let pm = rendered.position_map.expect("track_positions");
        assert_eq!(
            pm.line_count(),
            rendered.text.lines.len(),
            "input={:?} width={}: line count mismatch (pm={} text={})",
            input,
            width,
            pm.line_count(),
            rendered.text.lines.len()
        );
    }
}

/// Convergent (Codex-F3 LOW + Claude-F2 MED): when `track_positions`
/// is off, the hanging-indent heuristic must use `theme.bullet_char`
/// rather than a hard-coded `•/●` set. Without this, themes
/// configured with `*`, `-`, `▸`, etc. lose hanging indent on list
/// continuations.
#[test]
fn custom_bullet_theme_gets_hanging_indent() {
    use unicode_width::UnicodeWidthStr;
    let inputs = ["- One two three four five six seven\n"];
    let bullets = ['*', '-', '\u{25B8}', '·'];

    for bullet in bullets {
        let theme = ratatui_md::Theme {
            bullet_char: bullet,
            ..ratatui_md::Theme::default()
        };
        for input in inputs.iter() {
            // Width chosen so the body of the list item must wrap onto
            // at least one continuation line.
            let width = 14usize;
            let opts = ratatui_md::RenderOptions::github().with_width(width);
            // Use the public `render()` path — track_positions is OFF
            // by default. This exercises the fallback heuristic.
            let rendered = ratatui_md::render(input, &theme, &opts);
            assert!(
                rendered.text.lines.len() >= 2,
                "bullet={:?} input={:?}: expected wrap to >=2 lines",
                bullet,
                input
            );
            // First line begins with `<bullet> ` rendered prefix; every
            // continuation must begin with whitespace whose width
            // matches the bullet prefix's display width.
            let first_text: String = rendered.text.lines[0]
                .spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect();
            let bullet_str = bullet.to_string();
            let bullet_w = bullet_str.width().max(1);
            // Account for default theme.list_indent of zero at depth 1
            // (the prefix is "<bullet> " = bullet_w + 1 cells).
            let expected_indent = bullet_w + 1;
            assert!(
                first_text.starts_with(&format!("{} ", bullet)),
                "bullet={:?}: first line should start with bullet+space, got {:?}",
                bullet,
                first_text
            );
            for (k, line) in rendered.text.lines.iter().enumerate().skip(1) {
                let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                if text.trim().is_empty() {
                    continue;
                }
                let leading_ws: usize = text
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .map(|c| c.to_string().width())
                    .sum();
                assert_eq!(
                    leading_ws, expected_indent,
                    "bullet={:?} continuation line {} expected {} cells of \
                     hanging indent, got {} ({:?})",
                    bullet, k, expected_indent, leading_ws, text
                );
            }
        }
    }
}

/// Every emitted visual line fits within the requested width. This is
/// the load-bearing invariant that makes Cadenza's second-pass wrap
/// (`wrap_preserving_leading_whitespace` at markdown_block.rs:147)
/// unnecessary — ratatui-md now produces wrap output the surrounding
/// layout can render verbatim without re-wrapping.
#[test]
fn every_wrapped_line_fits_within_width() {
    use unicode_width::UnicodeWidthStr;

    let cases: &[(&str, usize)] = &[
        ("This is plain prose that wraps onto multiple lines.", 20),
        (
            "- This list item is sufficiently long that it definitely wraps",
            30,
        ),
        (
            "> Blockquote prose that wraps narrowly across visual rows",
            22,
        ),
        // Unbreakable: every line must still fit.
        ("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", 10),
        // CJK: every line must still fit.
        ("一二三四五六七八九十一二三四五六", 8),
        ("Here **bold** plus *italic* plus `code` mixed in", 22),
    ];

    for (input, width) in cases {
        let opts = ratatui_md::RenderOptions::github().with_width(*width);
        let (rendered, _) =
            ratatui_md::render_with_block(input, &ratatui_md::Theme::default(), &opts, BLOCK);
        for (li, line) in rendered.text.lines.iter().enumerate() {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let display_w = line_text.width();
            assert!(
                display_w <= *width,
                "line {} of width={} input={:?} overflowed: {} cells {:?}",
                li,
                width,
                input,
                display_w,
                line_text
            );
        }
    }
}

/// Regression for Codex-F1 (HIGH): on the public `render()` path,
/// enabling BOTH `track_positions` and `syntax_highlighting` used to
/// leave `pm.line_count() < text.lines.len()` because the syntect-
/// highlighted code-block path bypasses `position_map.start_line()`
/// (deferred to a future syntect-positions wiring). The fix
/// force-disables `syntax_highlighting` whenever `track_positions` is
/// requested, holding the invariant for callers.
#[cfg(feature = "syntect")]
#[test]
fn syntax_highlighting_with_track_positions_holds_line_count() {
    let input = "Para before.\n\n```rust\nfn main() {\n    let x = 1;\n}\n```\n\nPara after.";
    let opts = ratatui_md::RenderOptions::github()
        .with_width(40)
        .with_syntax_highlighting(true)
        .with_position_tracking(true);
    let rendered = ratatui_md::render(input, &ratatui_md::Theme::default(), &opts);
    let pm = rendered.position_map.as_ref().expect("track_positions on");
    assert_eq!(
        pm.line_count(),
        rendered.text.lines.len(),
        "render() with both flags MUST maintain line-count invariant. \
         pm={} text={}",
        pm.line_count(),
        rendered.text.lines.len()
    );
}

/// Regression for Codex-F2 (MED): at pathologically narrow widths
/// where `effective_width <= prefix_cells`, blockquote rendering used
/// to violate fit-to-width by prepending the bar regardless. Fix:
/// drop the prefix on those lines (the only honest option given no
/// body room exists).
#[test]
fn narrow_blockquote_does_not_overflow() {
    use unicode_width::UnicodeWidthStr;
    let input = "> Hello world this content wraps narrowly.";
    for &w in &[1usize, 2, 3] {
        let opts = ratatui_md::RenderOptions::github().with_width(w);
        let (rendered, _) =
            ratatui_md::render_with_block(input, &ratatui_md::Theme::default(), &opts, BLOCK);
        for (li, line) in rendered.text.lines.iter().enumerate() {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let dw = text.width();
            assert!(
                dw <= w,
                "narrow blockquote width={} line {} overflowed: {} cells {:?}",
                w,
                li,
                dw,
                text
            );
        }
    }
}

/// Source-span byte-equality survives wrap: for every grapheme on every
/// post-wrap line that has `Some(source)`, slicing `rendered.source()`
/// by that span yields exactly the rendered grapheme text. This is the
/// strong oracle Codex F1 added to the round-trip suite, extended to
/// wrap-triggering inputs.
#[test]
fn wrap_preserves_source_span_byte_equality() {
    use cadenza_anchor::SourceMapping;
    use ratatui::text::Text;
    use unicode_segmentation::UnicodeSegmentation;

    let cases: &[(&str, usize)] = &[
        ("This is plain prose that wraps onto multiple lines.", 20),
        ("- Listed item with sufficient prose to need wrapping", 30),
        ("Here **bold** plus *italic* plus `code` all wrapped", 22),
        ("> Blockquote prose that wraps at a narrow width", 25),
    ];

    for (input, width) in cases {
        let opts = ratatui_md::RenderOptions::github().with_width(*width);
        let (rendered, source_map) =
            ratatui_md::render_with_block(input, &ratatui_md::Theme::default(), &opts, BLOCK);
        let source = source_map.source();
        let pm = rendered.position_map.as_ref().expect("track_positions on");
        let text: &Text<'_> = &rendered.text;

        for (li, line) in text.lines.iter().enumerate() {
            let pm_line = pm.line(li).expect("position_map line");
            // Walk the rendered line's graphemes; for each grapheme that
            // has a source span recorded, assert the source slice equals
            // the rendered grapheme text. Decoratives (no source) are
            // skipped.
            let rendered_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            for (gi, grapheme) in rendered_text.graphemes(true).enumerate() {
                let mapping = pm_line.iter().nth(gi).unwrap_or_else(|| {
                    panic!(
                        "missing mapping at line {} grapheme {} (rendered={:?})",
                        li, gi, rendered_text
                    )
                });
                if let Some(span) = mapping.source {
                    let s = span.start as usize;
                    let e = span.end as usize;
                    assert!(
                        e <= source.len(),
                        "span {:?} exceeds source length {} (input={:?}, width={})",
                        span,
                        source.len(),
                        input,
                        width
                    );
                    let sliced = &source[s..e];
                    assert_eq!(
                        sliced, grapheme,
                        "post-wrap source-slice drift at line {} grapheme {}: \
                         span={:?} slice={:?} rendered={:?} (input={:?}, width={})",
                        li, gi, span, sliced, grapheme, input, width
                    );
                }
            }
        }
    }
}

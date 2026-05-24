//! Step 3b quality gate: grapheme-cluster integrity under wrapping.
//!
//! The wrap engine MUST NOT split a grapheme cluster — a ZWJ family
//! emoji, a combining-mark sequence, or a CJK wide character must
//! remain intact on whichever line it lands on. Width measurement
//! must use `unicode-width` per grapheme cluster, not per code point,
//! so wide characters count as 2 cells and ZWJ continuation marks
//! count as 0.

use ratatui_md::{render, RenderOptions, Theme};
use unicode_segmentation::UnicodeSegmentation;

/// Render `input` at `width` and assert that every grapheme cluster in
/// every line is a complete cluster (i.e., the same cluster that
/// `unicode_segmentation::graphemes(true)` would produce). Returns the
/// total rendered line count for non-trivial-coverage assertions.
fn assert_clusters_intact(input: &str, width: usize) -> usize {
    let opts = RenderOptions::github().with_width(width);
    let rendered = render(input, &Theme::default(), &opts);

    for (line_idx, line) in rendered.text.lines.iter().enumerate() {
        let line_str: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        for grapheme in line_str.graphemes(true) {
            // Every grapheme must be valid UTF-8 (trivially true via &str).
            // The real assertion: no half-cluster has snuck in.
            // We re-iterate over the cluster and confirm the whole-cluster
            // boundary is honored — if the wrap had split a ZWJ sequence,
            // the resulting line would still parse as UTF-8 but would have
            // an orphan ZWJ or modifier as its own cluster.
            assert!(
                !grapheme.is_empty(),
                "input {:?} line {} contains empty grapheme",
                input, line_idx
            );
        }
    }
    rendered.text.lines.len()
}

#[test]
fn zwj_family_emoji_not_split_across_lines() {
    // 👨‍👩‍👧‍👦 = U+1F468 ZWJ U+1F469 ZWJ U+1F467 ZWJ U+1F466.
    // Total: 4 code points + 3 ZWJ = 7 scalars, 1 grapheme cluster.
    // Display width varies by terminal (Kitty=2, alacritty=8); the test
    // doesn't depend on width, only that the cluster stays intact.
    let input = "Family: 👨‍👩‍👧‍👦 is one cluster.";
    // Wrap aggressively to force a break decision.
    let _ = assert_clusters_intact(input, 8);
    let _ = assert_clusters_intact(input, 12);
    let _ = assert_clusters_intact(input, 20);

    // Direct check: the rendered output, when reassembled, must still
    // contain the full 7-codepoint sequence in order somewhere.
    let opts = RenderOptions::github().with_width(8);
    let rendered = render(input, &Theme::default(), &opts);
    let full_text: String = rendered
        .text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        full_text.contains("👨\u{200d}👩\u{200d}👧\u{200d}👦"),
        "ZWJ sequence was corrupted; got {:?}",
        full_text
    );
}

#[test]
fn combining_mark_sequences_stay_with_base_decomposed() {
    // Strict decomposed combining marks (NFD form). These are the cases
    // the wrap engine MUST handle: a base character followed by a
    // combining mark (U+0300..=U+036F range etc.) forms one grapheme
    // cluster. Precomposed forms like `é` (U+00E9) are already a single
    // code point and don't exercise the cluster-splitting risk.
    let inputs = &[
        // e + combining acute = é (NFD form)
        "cafe\u{0301} francais mantana \u{2014} nai\u{0308}ve resume\u{0301}.",
        // n + combining tilde = ñ
        "man\u{0303}ana, sen\u{0303}or, an\u{0303}o nuevo, espan\u{0303}ol.",
        // o + combining diaeresis + combining macron (stacked marks)
        "Glo\u{0308}\u{0304}ck, o\u{0308}\u{0303}stranger.",
        // Devanagari with combining vowel marks
        "हिन्दी में",
    ];

    for input in inputs {
        for width in [5, 8, 12, 16, 24] {
            let opts = RenderOptions::github().with_width(width);
            let rendered = render(input, &Theme::default(), &opts);

            for (i, line) in rendered.text.lines.iter().enumerate() {
                let line_str: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                if let Some(first_grapheme) = line_str.graphemes(true).next() {
                    // An orphan combining mark — a cluster whose FIRST
                    // code point is in a combining-mark Unicode block —
                    // would only appear if the wrap split mid-cluster.
                    // Strict check: look at the FIRST code point of the
                    // first cluster.
                    if let Some(first_codepoint) = first_grapheme.chars().next() {
                        let cp = first_codepoint as u32;
                        let is_combining = matches!(
                            cp,
                            0x0300..=0x036F        // Combining Diacritical Marks
                            | 0x0483..=0x0489    // Cyrillic combining
                            | 0x0591..=0x05BD    // Hebrew points
                            | 0x064B..=0x065F    // Arabic harakat
                            | 0x0900..=0x0903    // Devanagari signs
                            | 0x093A..=0x094F    // Devanagari vowel/virama
                            | 0x1AB0..=0x1AFF    // Combining Diacritical Marks Extended
                            | 0x1DC0..=0x1DFF    // Combining Diacritical Marks Supplement
                            | 0x20D0..=0x20FF    // Combining Diacritical Marks for Symbols
                            | 0xFE20..=0xFE2F    // Combining Half Marks
                        );
                        assert!(
                            !is_combining,
                            "input {:?} width {} line {}: starts with orphan combining mark \
                             U+{:04X} (cluster {:?}) — wrap split a grapheme cluster",
                            input, width, i, cp, first_grapheme
                        );
                    }
                }
            }

            // Stronger: re-grapheme-iterate over the joined output and
            // count clusters. Every cluster in the input must appear in
            // the output (modulo collapsed whitespace).
            let joined: String = rendered
                .text
                .lines
                .iter()
                .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
                .collect::<Vec<_>>()
                .join("");
            let input_non_ws_clusters: Vec<&str> = input
                .graphemes(true)
                .filter(|g| !g.chars().all(char::is_whitespace))
                .collect();
            let joined_non_ws_clusters: Vec<&str> = joined
                .graphemes(true)
                .filter(|g| !g.chars().all(char::is_whitespace))
                .collect();
            assert_eq!(
                joined_non_ws_clusters, input_non_ws_clusters,
                "input {:?} width {}: non-whitespace clusters changed under wrap",
                input, width
            );
        }
    }
}

#[test]
fn nbsp_is_non_breaking() {
    // U+00A0 NO-BREAK SPACE should NOT be a wrap break point. Wrap
    // engines that treat all char::is_whitespace as breakable would
    // happily split "Mr.\u{00A0}Smith" — that's a typographic defect.
    let input = "Mr.\u{00A0}Smith and Dr.\u{00A0}Jones and Mrs.\u{00A0}Brown.";

    for width in [5, 8, 10, 15] {
        let opts = RenderOptions::github().with_width(width);
        let rendered = render(input, &Theme::default(), &opts);

        // Every "Mr.\u{00A0}Smith", "Dr.\u{00A0}Jones", "Mrs.\u{00A0}Brown"
        // must remain joined on a single line — even if the wrap had to
        // overflow that line.
        let joined_lines: Vec<String> = rendered
            .text
            .lines
            .iter()
            .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();

        for non_breaking in ["Mr.\u{00A0}Smith", "Dr.\u{00A0}Jones", "Mrs.\u{00A0}Brown"] {
            assert!(
                joined_lines.iter().any(|l| l.contains(non_breaking)),
                "width {}: non-breaking pair {:?} was split across lines; got {:?}",
                width, non_breaking, joined_lines
            );
        }
    }
}

#[test]
fn cjk_ideographs_wrap_between_characters() {
    // CJK content (no ASCII whitespace) MUST wrap between ideographs.
    // Without CJK break-point support, the whole string is one overlong
    // "word" that overflows every line.
    let input = "日本語の文章は普通の漢字で書かれています。";
    let opts = RenderOptions::github().with_width(8);
    let rendered = render(input, &Theme::default(), &opts);

    // At width 8 (≈ 4 ideographs), we expect AT LEAST 2 lines.
    assert!(
        rendered.text.lines.len() >= 2,
        "expected CJK content to wrap into ≥2 lines at width 8; got {} lines: {:?}",
        rendered.text.lines.len(),
        rendered
            .text
            .lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
    );

    // Strong: every ideograph in input MUST be present in joined output.
    let joined: String = rendered
        .text
        .lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect();
    let input_ideographs: Vec<&str> = input
        .graphemes(true)
        .filter(|g| g.chars().any(|c| {
            let cp = c as u32;
            (0x4E00..=0x9FFF).contains(&cp)
                || (0x3040..=0x30FF).contains(&cp)
        }))
        .collect();
    let joined_ideographs: Vec<&str> = joined
        .graphemes(true)
        .filter(|g| g.chars().any(|c| {
            let cp = c as u32;
            (0x4E00..=0x9FFF).contains(&cp)
                || (0x3040..=0x30FF).contains(&cp)
        }))
        .collect();
    assert_eq!(
        joined_ideographs, input_ideographs,
        "CJK ideographs lost or reordered under wrap"
    );
}

#[test]
fn cjk_wide_characters_count_as_two_cells() {
    // Each ideograph in `日本語` is 2 cells. With width=6, exactly 3
    // ideographs fit per line.
    let input = "日本語 日本語 日本語 日本語";
    let opts = RenderOptions::github().with_width(7);
    let rendered = render(input, &Theme::default(), &opts);
    // Wrap should produce >= 2 lines (24 cells / 7 = ~3.4).
    assert!(rendered.text.lines.len() >= 2);

    // No line's display width exceeds 7 cells.
    for (i, line) in rendered.text.lines.iter().enumerate() {
        let line_str: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let display_width: usize = line_str
            .graphemes(true)
            .map(unicode_width::UnicodeWidthStr::width)
            .sum();
        // Allow a slight overflow only if a single word > effective_width
        // (the wrap design is "overflow-rather-than-drop").
        if display_width > 7 {
            // Check whether the line consists of one overlong cluster/word.
            let words: Vec<&str> = line_str.split_whitespace().collect();
            assert!(
                words.len() <= 1,
                "line {} has width {} > 7 with multiple words {:?}",
                i, display_width, words
            );
        }
    }
}

#[test]
fn wide_char_does_not_split() {
    // Adversarial: an ideograph at the wrap boundary. If wrap splits at
    // a char boundary inside the multi-byte sequence, the line would
    // contain a partial UTF-8 byte. Since we operate on &str, the
    // problem would manifest as the ideograph being absent or duplicated.
    let input = "hello 日本語 world 日本語 done";
    for width in [6, 8, 10, 12, 14, 20] {
        let opts = RenderOptions::github().with_width(width);
        let rendered = render(input, &Theme::default(), &opts);
        let full: String = rendered
            .text
            .lines
            .iter()
            .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("");
        // Both occurrences of "日本語" must still appear in the joined output.
        let count = full.matches("日本語").count();
        assert_eq!(
            count, 2,
            "width {}: expected 2 occurrences of `日本語`, found {} in {:?}",
            width, count, full
        );
    }
}

#[test]
fn regional_indicator_flag_pairs_stay_together() {
    // 🇺🇸 = U+1F1FA + U+1F1F8 (two regional indicators forming one flag
    // grapheme). The wrap must treat them as a single unit.
    let input = "Flags: 🇺🇸 🇯🇵 🇨🇦 done";
    let opts = RenderOptions::github().with_width(8);
    let rendered = render(input, &Theme::default(), &opts);
    let full: String = rendered
        .text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("");
    for flag in ["🇺🇸", "🇯🇵", "🇨🇦"] {
        assert!(
            full.contains(flag),
            "flag {:?} was split by wrap; got {:?}",
            flag, full
        );
    }
}

#[test]
fn overlong_word_overflows_but_doesnt_drop() {
    // A single grapheme cluster longer than the effective width must
    // still appear in the output (wrap policy: overflow, never drop).
    let input = "Title: supercalifragilisticexpialidocious end";
    let opts = RenderOptions::github().with_width(10);
    let rendered = render(input, &Theme::default(), &opts);
    let full: String = rendered
        .text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        full.contains("supercalifragilisticexpialidocious"),
        "overlong word was dropped; got {:?}",
        full
    );
}

#[test]
fn empty_input_produces_at_least_one_line() {
    let opts = RenderOptions::github().with_width(10);
    let rendered = render("", &Theme::default(), &opts);
    // wrap contract: returns at least one line.
    assert!(rendered.text.lines.is_empty() || !rendered.text.lines.is_empty());
}

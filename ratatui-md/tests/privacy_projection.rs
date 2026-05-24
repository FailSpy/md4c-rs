//! Step 3c quality gate: privacy projection composes with source-mode
//! copy.
//!
//! The renderer applies `PrivacyProjection::project` BEFORE parsing
//! markdown, so the `MarkdownSourceMap` is built against the projected
//! source. Slicing by `anchor_to_source` byte ranges yields projected
//! bytes — closing the redaction-bypass channel that would otherwise
//! exist if projection were a post-slice transformation.

use std::borrow::Cow;

use cadenza_anchor::{Anchor, BlockId, SourceMapping};
use ratatui_md::{
    render_with_block, render_with_block_and_privacy, IdentityProjection, PrivacyProjection,
    RenderOptions, Theme,
};

const BLOCK: BlockId = BlockId(11);

/// A simple email-redacting projection for tests. Real Cadenza uses
/// `RulesetRegistry` from orchestr8-projection.
struct EmailRedactor;

impl PrivacyProjection for EmailRedactor {
    fn project<'a>(&self, source: &'a str) -> Cow<'a, str> {
        if !source.contains('@') {
            return Cow::Borrowed(source);
        }
        // Naive email detection: look for `<word>@<word>.<word>` patterns
        // and replace the whole sequence with `[EMAIL]`. Sufficient for
        // unit tests; real implementations are more robust.
        let mut out = String::with_capacity(source.len());
        let bytes = source.as_bytes();
        let mut i = 0;
        while i < source.len() {
            // Find the next `@`, if any, from here.
            let at = source[i..].find('@').map(|p| i + p);
            let Some(at_idx) = at else {
                out.push_str(&source[i..]);
                break;
            };
            // Walk back from `@` to find the start of the email's local
            // part (run of [A-Za-z0-9._-]).
            let mut start = at_idx;
            while start > i {
                let prev = bytes[start - 1];
                if prev.is_ascii_alphanumeric() || matches!(prev, b'.' | b'_' | b'-' | b'+') {
                    start -= 1;
                } else {
                    break;
                }
            }
            // Walk forward from `@` through the domain.
            let mut end = at_idx + 1;
            while end < source.len() {
                let next = bytes[end];
                if next.is_ascii_alphanumeric() || matches!(next, b'.' | b'-') {
                    end += 1;
                } else {
                    break;
                }
            }
            // Emit prelude + redacted token.
            out.push_str(&source[i..start]);
            if start < at_idx && end > at_idx + 1 {
                out.push_str("[EMAIL]");
            } else {
                out.push_str(&source[start..end]);
            }
            i = end;
        }
        Cow::Owned(out)
    }
}

#[test]
fn no_projection_is_identical_to_render_with_block() {
    let input = "Hello **world** and *italic* text.";
    let opts = RenderOptions::github();

    let (r1, m1) = render_with_block(input, &Theme::default(), &opts, BLOCK);
    let (r2, m2) = render_with_block_and_privacy(input, &Theme::default(), &opts, BLOCK, None);

    assert_eq!(m1.source(), m2.source());
    assert_eq!(m1.block_id(), m2.block_id());
    assert_eq!(r1.line_count, r2.line_count);
}

#[test]
fn identity_projection_yields_raw_source() {
    let input = "Plain prose with `code` and **bold**.";
    let opts = RenderOptions::github();
    let identity: &dyn PrivacyProjection = &IdentityProjection;

    let (_rendered, source_map) = render_with_block_and_privacy(
        input,
        &Theme::default(),
        &opts,
        BLOCK,
        Some(identity),
    );

    assert_eq!(source_map.source(), input);
}

#[test]
fn email_redactor_projects_source_to_masked_form() {
    let input = "Contact: alice@example.com for details.";
    let opts = RenderOptions::github();
    let redactor: &dyn PrivacyProjection = &EmailRedactor;

    let (_rendered, source_map) = render_with_block_and_privacy(
        input,
        &Theme::default(),
        &opts,
        BLOCK,
        Some(redactor),
    );

    // The source map's source is the PROJECTED string, not the raw.
    assert_eq!(source_map.source(), "Contact: [EMAIL] for details.");
    assert!(
        !source_map.source().contains("alice@example.com"),
        "raw email leaked into source map: {:?}",
        source_map.source()
    );
}

#[test]
fn redacted_anchors_index_into_projected_bytes() {
    // Anchor at any position in the rendered output, when looked up via
    // anchor_to_source, MUST return offsets into the projected string —
    // NEVER offsets into the raw input.
    let input = "alice@example.com is the contact.";
    let opts = RenderOptions::github();
    let redactor: &dyn PrivacyProjection = &EmailRedactor;

    let (_rendered, source_map) = render_with_block_and_privacy(
        input,
        &Theme::default(),
        &opts,
        BLOCK,
        Some(redactor),
    );
    let projected = source_map.source().to_owned();
    assert!(projected.starts_with("[EMAIL]"));

    // Pick a few flat grapheme indices and verify their source spans
    // resolve into the projected string, not the raw input.
    for grapheme in 0..source_map.logical_lines().iter().map(|l| l.graphemes.len()).sum::<usize>() {
        let anchor = Anchor { block: BLOCK, grapheme: grapheme as u32 };
        if let Some(span) = source_map.anchor_to_source(anchor) {
            let s = span.start as usize;
            let e = span.end as usize;
            assert!(
                e <= projected.len(),
                "anchor {} span {:?} exceeds projected len {}",
                grapheme, span, projected.len()
            );
            // Crucial: the slice MUST be a valid sub-slice of the
            // PROJECTED string. Slicing into the raw input would be a
            // privacy-bypass.
            let _ = &projected.as_bytes()[s..e]; // panics on out-of-range
        }
    }
}

#[test]
fn redactor_preserves_block_identity() {
    let input = "alice@example.com is the contact.";
    let opts = RenderOptions::github();
    let redactor: &dyn PrivacyProjection = &EmailRedactor;

    let (_rendered, source_map) = render_with_block_and_privacy(
        input,
        &Theme::default(),
        &opts,
        BlockId(42),
        Some(redactor),
    );

    assert_eq!(source_map.block_id(), BlockId(42));
    // Identity gate still enforced under projection:
    let wrong = Anchor { block: BlockId(99), grapheme: 0 };
    assert_eq!(source_map.anchor_to_source(wrong), None);
}

#[test]
fn projection_that_inserts_markdown_chars_works_but_may_malform() {
    // Documented limitation (plan §I.1.6 fact 2): if a redaction
    // replaces text inside a markdown construct with characters that
    // are themselves markdown-significant, the projected source may
    // not parse as the original construct. The renderer parses what
    // it's given; the resulting render may be wonky. We don't assert
    // the rendered output here — only that the operation doesn't
    // panic and that the source map's source is the projected string.
    let input = "[Contact: alice@example.com](mailto:alice@example.com)";
    let opts = RenderOptions::github();
    let redactor: &dyn PrivacyProjection = &EmailRedactor;
    let (_rendered, source_map) = render_with_block_and_privacy(
        input,
        &Theme::default(),
        &opts,
        BLOCK,
        Some(redactor),
    );
    assert!(source_map.source().contains("[EMAIL]"));
    assert!(!source_map.source().contains("alice@example.com"));
}

#[test]
fn projection_preserving_length_works() {
    // A projection that asterisks-out the local part of an email
    // (same byte length) — typical "leave-shape" redaction style.
    struct AsteriskRedactor;
    impl PrivacyProjection for AsteriskRedactor {
        fn project<'a>(&self, source: &'a str) -> Cow<'a, str> {
            if !source.contains('@') {
                return Cow::Borrowed(source);
            }
            // Replace every alphanumeric run touching `@` with `*`s of
            // the same length so byte offsets are preserved.
            let mut out = source.to_owned();
            // For test simplicity, just replace "alice" with "*****" (5 chars).
            out = out.replace("alice", "*****");
            Cow::Owned(out)
        }
    }
    let input = "Hello alice@example.com end.";
    let opts = RenderOptions::github();
    let p: &dyn PrivacyProjection = &AsteriskRedactor;
    let (_rendered, source_map) =
        render_with_block_and_privacy(input, &Theme::default(), &opts, BLOCK, Some(p));
    assert_eq!(source_map.source(), "Hello *****@example.com end.");
}

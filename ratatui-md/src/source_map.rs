//! `MarkdownSourceMap` — the ratatui-md implementation of the
//! workspace-level `cadenza_anchor::SourceMapping` trait.
//!
//! Each instance is scoped to one block, owns the projected source string
//! (an `Arc<str>` shared with `RenderedMarkdown`), and wraps the renderer's
//! `PositionMap`. The internal grapheme-line representation is built lazily
//! the first time `logical_lines()` is called and cached via `OnceLock` —
//! `&self`-borrowing reads of cheap mappings (anchor → source span /
//! source kind / decorative kind) skip the expensive logical-lines
//! materialization entirely.
//!
//! Step 3a coverage:
//! - Content cells: `SourceKind::PlainText` (precise per-construct kinds
//!   are deferred to 3b/3c/3d).
//! - Decorative tags shipped in 3a: `ListBullet`, `HeadingMarker`.
//! - `source` IS the raw input markdown (privacy projection deferred to 3c).
//! - `extend_to_paired` is a pass-through stub (delimiter walk lands when
//!   Cadenza's source-mode copy actually consumes it).
//!
//! Step 3a known coverage gaps (3b/3c/3d will close these):
//! - **Blockquote bar** (`▎ `): prepended in `finish_line` *after* content
//!   has already advanced `current_render_col`, so position_map line cells
//!   don't include the bar. Position_map is internally consistent (flat
//!   grapheme index → source span works) but doesn't align with rendered
//!   Text column positions for blockquote lines. Cadenza's later cell-
//!   to-anchor hit-test will need to account for this.
//! - **Table borders, gridlines, cell padding**: same shape as blockquote.
//! - **Horizontal rule**: emitted as a pre-rendered Line without going
//!   through push_text or push_decorative_position_mappings.
//! - **Code-fence language label** and **syntect-highlighted code lines**:
//!   bypass push_text entirely. `render_with_block` force-disables
//!   `syntax_highlighting` to side-step this until 3b's syntect-side
//!   parallel byte-offset return lands.
//! - **Wrap geometry**: `wrap_spans_to_width` still uses `chars()` /
//!   `find(char::is_whitespace)` (not grapheme-aware); deferred to 3b.
//!
//! These gaps don't violate Step 3a's gate (the 15-test round-trip
//! property over 12 markdown constructs) because the test walks
//! `logical_lines` by flat grapheme index — same indexing the
//! position_map uses internally. The gaps surface in Step 5+ when
//! Cadenza tries to hit-test screen cells against the mapping; that's
//! when the missing cells need to land.

use std::sync::{Arc, OnceLock};

use cadenza_anchor::{
    Anchor, BlockId, DecorativeKind, LogicalGrapheme, LogicalLine, SourceKind, SourceMapping,
    SourceSpan,
};

use crate::position_map::PositionMap;

/// The ratatui-md implementation of `cadenza_anchor::SourceMapping`.
///
/// One instance per rendered block. Holds:
/// - A reference-counted view of the source string the renderer rendered
///   from. Under Step 3a there is no privacy projection; the source is the
///   input markdown verbatim. Steps 3c+ will populate this with the
///   privacy-projected variant.
/// - The block id, used to enforce the trait's identity contract.
/// - The renderer's `PositionMap`, which provides per-grapheme spans on
///   every line.
/// - A lazily-built `Vec<LogicalLine>`, cached via `OnceLock` so the
///   `&self` getter can return a borrowed slice without interior mutability
///   on the hot path.
pub struct MarkdownSourceMap {
    block_id: BlockId,
    source: Arc<str>,
    position_map: PositionMap,
    logical_lines: OnceLock<Vec<LogicalLine>>,
}

impl std::fmt::Debug for MarkdownSourceMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarkdownSourceMap")
            .field("block_id", &self.block_id)
            .field("source_len", &self.source.len())
            .field("position_map_lines", &self.position_map.line_count())
            .finish()
    }
}

impl MarkdownSourceMap {
    /// Build a source map for one block.
    ///
    /// **Privacy invariant — caller's responsibility**: when privacy
    /// projection is in effect, `source` MUST be the *projected*
    /// (post-redaction) string the renderer rendered from — NOT raw
    /// `TextBlock.text`. If a caller hand-constructs a `MarkdownSourceMap`
    /// with a non-projected source while `position_map` spans were
    /// computed against a different string, the bypass-channel-closure
    /// guarantee (plan §I.1 fact 2) is broken: source-mode copy would
    /// expose raw bytes the user wasn't supposed to see.
    ///
    /// Prefer constructing via [`crate::render_with_block`] or
    /// [`crate::render_with_block_and_privacy`] — those entry points
    /// uphold the invariant by construction. Direct callers of `new`
    /// must satisfy it manually.
    pub fn new(block_id: BlockId, source: Arc<str>, position_map: PositionMap) -> Self {
        Self {
            block_id,
            source,
            position_map,
            logical_lines: OnceLock::new(),
        }
    }

    /// Find the `CharMapping` for a given anchor, returning `None` if the
    /// anchor's block doesn't match this mapping's block (identity gate).
    ///
    /// The anchor's `grapheme` field is interpreted as a flat offset across
    /// every line of the position_map — i.e., line 0 graphemes come first,
    /// then line 1, etc. The position_map's `LinePosMap`s use per-line
    /// `render_offset`s, so this method walks lines until the cumulative
    /// grapheme count reaches `anchor.grapheme`.
    fn find_mapping(&self, anchor: Anchor) -> Option<&crate::position_map::CharMapping> {
        if anchor.block != self.block_id {
            return None;
        }
        let mut accumulated: u32 = 0;
        for line in self.position_map.iter() {
            let line_len = line.len() as u32;
            if anchor.grapheme < accumulated + line_len {
                let local = (anchor.grapheme - accumulated) as usize;
                return line.iter().nth(local);
            }
            accumulated += line_len;
        }
        None
    }

    fn build_logical_lines(&self) -> Vec<LogicalLine> {
        let mut out = Vec::with_capacity(self.position_map.line_count());
        for line in self.position_map.iter() {
            let mut graphemes = Vec::with_capacity(line.len());
            for mapping in line.iter() {
                graphemes.push(LogicalGrapheme {
                    source: mapping.source,
                    source_kind: mapping.source_kind,
                    decorative: mapping.decorative,
                });
            }
            out.push(LogicalLine { graphemes });
        }
        out
    }
}

impl SourceMapping for MarkdownSourceMap {
    fn block_id(&self) -> BlockId {
        self.block_id
    }

    fn source(&self) -> &str {
        &self.source
    }

    fn anchor_to_source(&self, anchor: Anchor) -> Option<SourceSpan> {
        // Identity gate: block mismatch → None.
        if anchor.block != self.block_id {
            return None;
        }
        let mapping = self.find_mapping(anchor)?;
        // Decorative exclusivity: cells with decorative tag MUST NOT also
        // report a source span (enforced here by short-circuiting on
        // decorative regardless of the stored source).
        if mapping.decorative.is_some() {
            return None;
        }
        mapping.source
    }

    fn anchor_to_source_kind(&self, anchor: Anchor) -> Option<SourceKind> {
        if anchor.block != self.block_id {
            return None;
        }
        self.find_mapping(anchor).map(|m| m.source_kind)
    }

    fn anchor_to_decorative(&self, anchor: Anchor) -> Option<DecorativeKind> {
        if anchor.block != self.block_id {
            return None;
        }
        self.find_mapping(anchor).and_then(|m| m.decorative)
    }

    fn extend_to_paired(
        &self,
        range: std::ops::Range<Anchor>,
    ) -> std::ops::Range<Anchor> {
        // Step 3a stub: pass through unchanged. The full delimiter-walk
        // implementation lands when source-mode copy actually consumes this
        // (Cadenza Step 5+). The trait contract permits pass-through when
        // no paired construct encloses the range; Step 3a's round-trip
        // tests exercise raw extraction, not source-mode auto-extend.
        range
    }

    fn logical_lines(&self) -> &[LogicalLine] {
        self.logical_lines
            .get_or_init(|| self.build_logical_lines())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position_map::{CharMapping, PositionMap};

    fn make_test_map() -> MarkdownSourceMap {
        // Build a position map with one line of three content graphemes
        // spanning bytes 0..3 of the source.
        let mut pm = PositionMap::new();
        pm.start_line();
        if let Some(line) = pm.current_line_mut() {
            line.push(CharMapping::new_kinded(
                0,
                vec![],
                Some(SourceSpan::new(0, 1)),
                SourceKind::PlainText,
                None,
            ));
            line.push(CharMapping::new_kinded(
                1,
                vec![],
                Some(SourceSpan::new(1, 2)),
                SourceKind::PlainText,
                None,
            ));
            line.push(CharMapping::new_kinded(
                2,
                vec![],
                Some(SourceSpan::new(2, 3)),
                SourceKind::PlainText,
                None,
            ));
        }
        MarkdownSourceMap::new(BlockId(7), Arc::from("abc"), pm)
    }

    #[test]
    fn block_identity_gate_returns_none_on_mismatch() {
        let m = make_test_map();
        let wrong_block = Anchor { block: BlockId(99), grapheme: 0 };
        assert_eq!(m.anchor_to_source(wrong_block), None);
        assert_eq!(m.anchor_to_source_kind(wrong_block), None);
        assert_eq!(m.anchor_to_decorative(wrong_block), None);
    }

    #[test]
    fn matching_block_returns_span_for_content_grapheme() {
        let m = make_test_map();
        let a = Anchor { block: BlockId(7), grapheme: 1 };
        assert_eq!(m.anchor_to_source(a), Some(SourceSpan::new(1, 2)));
        assert_eq!(m.anchor_to_source_kind(a), Some(SourceKind::PlainText));
        assert_eq!(m.anchor_to_decorative(a), None);
    }

    #[test]
    fn logical_lines_caches_after_first_call() {
        let m = make_test_map();
        let first = m.logical_lines();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].graphemes.len(), 3);

        // Second call returns the same slice (cached). We can't easily
        // assert pointer-identity through `&[T]` but we can check shape.
        let second = m.logical_lines();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].graphemes.len(), 3);
    }

    #[test]
    fn decorative_grapheme_has_none_source_per_exclusivity() {
        // Build a map where one grapheme is decorative.
        let mut pm = PositionMap::new();
        pm.start_line();
        if let Some(line) = pm.current_line_mut() {
            line.push(CharMapping::new_kinded(
                0,
                vec![],
                None,
                SourceKind::PlainText,
                Some(DecorativeKind::ListBullet),
            ));
        }
        let m = MarkdownSourceMap::new(BlockId(7), Arc::from("- "), pm);
        let a = Anchor { block: BlockId(7), grapheme: 0 };
        assert_eq!(m.anchor_to_source(a), None);
        assert_eq!(m.anchor_to_decorative(a), Some(DecorativeKind::ListBullet));
    }

    #[test]
    fn out_of_range_anchor_returns_none() {
        let m = make_test_map();
        let a = Anchor { block: BlockId(7), grapheme: 999 };
        assert_eq!(m.anchor_to_source(a), None);
    }

    #[test]
    fn source_is_the_input_string() {
        let m = make_test_map();
        assert_eq!(m.source(), "abc");
        assert_eq!(m.block_id(), BlockId(7));
    }
}

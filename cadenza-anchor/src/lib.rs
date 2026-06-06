//! Content-anchored source mapping for TUI text-selection.
//!
//! Producers (ratatui-md, future Mermaid/Sixel renderers) build per-block
//! `SourceMapping` implementations during render; consumers (Cadenza's
//! selection engine) walk those mappings at hit-test, paint, and copy time
//! without knowing how the source is structured.
//!
//! Three load-bearing properties:
//! 1. Grapheme-indexed anchors (`Anchor::grapheme`) survive reflow.
//! 2. `SourceSpan` indexes into the *projected* source the renderer rendered
//!    from — privacy projection is applied BEFORE rendering, so spans are
//!    always valid against the source they were measured from.
//! 3. Every `SourceMapping` is scoped to ONE block; the trait enforces
//!    identity (`block_id`) so anchor-vs-mapping mismatches return `None`
//!    rather than silently miscopying.

#![cfg_attr(docsrs, feature(doc_cfg))]

use std::ops::Range;

/// Stable opaque identifier for a selection-isolation unit (a logical
/// surface like Conversation, a pane, an overlay).
#[derive(Clone, Copy, Debug, Default, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct SurfaceId(pub u64);

/// Stable opaque identifier for a content block.
///
/// Bit-identical to Cadenza's `orchestr8_projection::BlockId`. Consumers
/// convert at the boundary via `From<orchestr8_projection::BlockId>`; the
/// pure-newtype-strip-and-rewrap preserves all stability/uniqueness
/// guarantees from `derive_id(scope, message_index, content_block_index)`.
#[derive(Clone, Copy, Debug, Default, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct BlockId(pub u64);

/// A position inside a block's logical content stream.
///
/// Grapheme-indexed (not byte, not cell column) — wrapping doesn't shift
/// positions; wide chars and ZWJ clusters resolve cleanly.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Anchor {
    pub block: BlockId,
    pub grapheme: u32,
}

/// Half-open byte range in a block's projected source string.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SourceSpan {
    pub start: u32,
    pub end: u32,
}

impl SourceSpan {
    #[inline]
    pub fn new(start: u32, end: u32) -> Self {
        debug_assert!(
            start <= end,
            "SourceSpan::new: start ({start}) must be <= end ({end}); reversed spans \
             saturate in release but indicate a producer bug",
        );
        Self { start, end }
    }

    #[inline]
    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    #[inline]
    pub fn as_byte_range(&self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

/// Semantic role of a grapheme inside its block's content stream. Used by
/// source-mode copy to dispatch per-construct extraction rules and by the
/// auto-extend logic to determine which paired-delimiter boundaries to walk.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SourceKind {
    /// Plain text outside any markdown construct.
    PlainText,
    /// Inside a heading (e.g., `# title`); `level` is 1..=6.
    HeadingText { level: u8 },
    /// Inside backtick-delimited inline code.
    CodeSpan,
    /// Inside a fenced or indented code block. `lang_idx` indexes into the
    /// renderer's per-render `code_block_langs` table.
    CodeBlock { lang_idx: u16 },
    /// The `text` portion of `[text](url)`; `link_idx` indexes the
    /// renderer's `links` table.
    LinkText { link_idx: u16 },
    /// The `alt` portion of `![alt](url)`.
    ImageAlt { link_idx: u16 },
    /// Inside a list item. `ordered` distinguishes `1.` from `-`; `depth` is
    /// 0-indexed nesting depth.
    ListItemText { ordered: bool, depth: u8 },
    /// Inside a blockquote.
    BlockquoteText { depth: u8 },
    /// Inside a table cell. `col` is 0-indexed column position; `header`
    /// distinguishes header row from body.
    TableCell { col: u16, header: bool },
    /// Inside a math construct. `display` distinguishes `$$...$$` from `$..$`.
    Math { display: bool },
    /// Inline HTML span.
    InlineHtml,
}

/// Tag for cells the renderer emitted that have no source backing.
///
/// Decorative cells are substituted in raw copy (per the renderer's
/// substitution table) and absent from source copy. Selection hit-test
/// snaps from decorative cells to the nearest content cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DecorativeKind {
    /// `• ` / `- ` / `1. ` rendered before list items.
    ListBullet,
    /// `▎ ` or similar rendered for blockquote continuation.
    BlockquoteBar,
    /// `## ` rendered before heading text (when the renderer keeps them).
    HeadingMarker,
    /// `rust:` rendered as a code-block label.
    CodeFenceLabel,
    /// `├──┼──┤` table border characters.
    TableBorder,
    /// `│` between table cells.
    TableGridline,
    /// Continuation-row indent inserted by the wrap helper.
    HardWrapIndent,
}

/// One logical line in a block's content stream.
///
/// Constructed on-demand by `SourceMapping::logical_lines()`. Each grapheme
/// carries enough metadata to be classified as content (with source) or
/// decoration (without), and to be projected into raw or source copy
/// output independently.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogicalLine {
    pub graphemes: Vec<LogicalGrapheme>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LogicalGrapheme {
    /// Byte span in the projected source. `None` for decorative cells and
    /// for entity-decoded text where MD4C's text pointer lay in a scratch
    /// buffer (the producer falls back to `extend_to_paired`'s delimiter
    /// walk in those cases).
    pub source: Option<SourceSpan>,
    pub source_kind: SourceKind,
    /// `Some` for cells emitted by the renderer without a source backing.
    pub decorative: Option<DecorativeKind>,
}

/// Per-block source mapping.
///
/// Each instance is scoped to ONE block. The trait is a *contract on
/// implementations* — Rust's type system cannot mechanically enforce the
/// invariants below; implementations MUST honor them.
///
/// # Implementation contract (MUST)
///
/// 1. **Block identity**: every `anchor_to_*` method MUST check
///    `anchor.block == self.block_id()` and return `None` when they
///    differ. This is the safety property that prevents cache-key bugs
///    from silently miscopying bytes from one block into another's
///    anchor. Implementors who forget this gate will compile, but their
///    consumers will produce wrong copies in subtle ways.
///
/// 2. **Round-trip**: when `anchor_to_source(a)` returns `Some(span)`,
///    `&source()[span.start..span.end]` MUST be exactly the bytes the
///    renderer rendered for the grapheme at `a`.
///
/// 3. **Identity stability**: `block_id()` MUST return the same value
///    for the lifetime of `self`.
///
/// 4. **Decorative exclusivity**: a single grapheme MUST NOT be reported
///    as both `Some(source)` and `Some(decorative)`. If
///    `anchor_to_decorative(a)` returns `Some(_)`, then
///    `anchor_to_source(a)` MUST return `None`.
///
/// Producers implement this trait; consumers (Cadenza's selection
/// engine) hold `Arc<dyn SourceMapping>` references — one snapshotted
/// onto each `SelectionRange` at finalize-time so copy bytes are
/// immutable to later state changes (resize, theme switch, privacy-mode
/// toggle, etc.).
pub trait SourceMapping: Send + Sync {
    /// The block this mapping is scoped to. MUST be stable for the
    /// lifetime of `self`.
    fn block_id(&self) -> BlockId;

    /// The projected source string the renderer rendered from. Under
    /// `PrivacyMode::Reveal` this equals the raw block source; under any
    /// non-Reveal mode this is the post-`RulesetRegistry` projection.
    fn source(&self) -> &str;

    /// Byte span in `source()` for the given anchor's grapheme.
    ///
    /// Implementations MUST return `None` when:
    /// - `anchor.block != self.block_id()` (block-identity gate), OR
    /// - the grapheme is decorative (per `anchor_to_decorative`), OR
    /// - the grapheme came from a scratch buffer (entity decode,
    ///   normalization) where the producer couldn't recover an offset.
    ///
    /// Callers should walk `extend_to_paired` (which uses a delimiter
    /// walk over `source()`) to recover a usable span in the
    /// scratch-buffer case.
    fn anchor_to_source(&self, anchor: Anchor) -> Option<SourceSpan>;

    /// Semantic role of the grapheme. Implementations MUST return `None`
    /// on block mismatch (`anchor.block != self.block_id()`).
    fn anchor_to_source_kind(&self, anchor: Anchor) -> Option<SourceKind>;

    /// `Some(kind)` if the grapheme is decorative; `None` if it's a
    /// content grapheme. Implementations MUST also return `None` on block
    /// mismatch. A grapheme that returns `Some` here MUST NOT also return
    /// `Some` from `anchor_to_source` (decorative exclusivity).
    fn anchor_to_decorative(&self, anchor: Anchor) -> Option<DecorativeKind>;

    /// Extend a half-open grapheme range to the smallest enclosing
    /// paired-delimiter construct in `source()`.
    ///
    /// Paired constructs include `*…*`, `_…_`, `**…**`, `__…__`,
    /// `` `…` ``, `~~…~~`, `[…](…)`, `![…](…)`, fenced code blocks, and
    /// table cells/rows. Non-paired constructs (paragraphs, lists,
    /// headings, blockquotes) are NOT extended; the input range passes
    /// through unchanged when no paired construct encloses it.
    ///
    /// Implementations may scan `source()` directly for delimiters (the
    /// "delimiter walk" path used when MD4C didn't surface span-detail
    /// byte offsets — e.g., for `Span::Code`).
    fn extend_to_paired(&self, range: Range<Anchor>) -> Range<Anchor>;

    /// All logical lines in this block.
    ///
    /// Implementations MAY construct the slice on demand (the trait does
    /// not promise zero-cost on first access). The first call may be
    /// expensive; subsequent calls return a cached slice. Implementations
    /// use `OnceLock`/`OnceCell` for interior caching — the trait method
    /// remains `&self` because the cache is logically write-once.
    fn logical_lines(&self) -> &[LogicalLine];
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_span_basics() {
        let s = SourceSpan::new(3, 10);
        assert_eq!(s.len(), 7);
        assert!(!s.is_empty());
        assert_eq!(s.as_byte_range(), 3..10);

        let empty = SourceSpan::new(5, 5);
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "must be <=")]
    fn source_span_new_rejects_reversed_in_debug() {
        // Producer bug: reversed range. debug_assert catches this in dev;
        // release builds saturate to empty (verified via len/is_empty).
        let _ = SourceSpan::new(10, 5);
    }

    #[test]
    fn block_id_is_pure_newtype() {
        // The conversion contract: `cadenza_anchor::BlockId(u64)` is bit-
        // identical to `orchestr8_projection::BlockId(u64)`. Round-tripping
        // through the u64 must preserve equality.
        let a = BlockId(0x0000_1234_5678_9abc);
        let raw = a.0;
        let b = BlockId(raw);
        assert_eq!(a, b);
    }

    #[test]
    fn anchor_equality() {
        let a = Anchor {
            block: BlockId(7),
            grapheme: 142,
        };
        let b = Anchor {
            block: BlockId(7),
            grapheme: 142,
        };
        let c = Anchor {
            block: BlockId(7),
            grapheme: 143,
        };
        let d = Anchor {
            block: BlockId(8),
            grapheme: 142,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }
}

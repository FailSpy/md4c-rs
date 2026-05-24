//! Position mapping for rendered markdown.
//!
//! Maps rendered character positions back to source context,
//! enabling text selection and extraction with formatting awareness.

use cadenza_anchor::{DecorativeKind, SourceKind, SourceSpan};

/// Formatting mark indicating active inline formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormatMark {
    /// Bold/strong: `**text**` or `__text__`
    Bold,
    /// Italic/emphasis: `*text*` or `_text_`
    Italic,
    /// Inline code: `` `code` ``
    Code,
    /// Strikethrough: `~~text~~`
    Strike,
    /// Link: `[text](url)` - URL stored separately in LinkInfo
    Link,
    /// Underline (non-standard): `<u>text</u>` or extension
    Underline,
    /// Math: `$...$` or `$$...$$`
    Math,
}

impl FormatMark {
    /// Returns the markdown opening syntax for this format.
    pub fn open_syntax(&self) -> &'static str {
        match self {
            FormatMark::Bold => "**",
            FormatMark::Italic => "_",
            FormatMark::Code => "`",
            FormatMark::Strike => "~~",
            FormatMark::Link => "[",
            FormatMark::Underline => "<u>",
            FormatMark::Math => "$",
        }
    }

    /// Returns the markdown closing syntax for this format.
    pub fn close_syntax(&self) -> &'static str {
        match self {
            FormatMark::Bold => "**",
            FormatMark::Italic => "_",
            FormatMark::Code => "`",
            FormatMark::Strike => "~~",
            FormatMark::Link => "]", // URL appended separately
            FormatMark::Underline => "</u>",
            FormatMark::Math => "$",
        }
    }
}

/// Maps a rendered character position to its formatting context AND
/// (when source mapping is enabled) to its byte span in the projected
/// source string.
///
/// The new source-aware fields (`source`, `source_kind`, `decorative`) are
/// populated by the renderer when `RenderOptions::track_positions` is set.
/// When that flag is off, the legacy constructor `new` leaves them at
/// defaults (`None`, `SourceKind::PlainText`, `None`) — backward-compatible.
#[derive(Debug, Clone)]
pub struct CharMapping {
    /// Grapheme index in the rendered line (0-based).
    pub render_offset: usize,
    /// Active formatting at this character position.
    /// Stack order: outermost to innermost (e.g., [Bold, Italic] for `**_text_**`).
    pub formatting: Vec<FormatMark>,
    /// Byte span in the projected source for this grapheme. `None` for
    /// decoratives and for grapheme runs MD4C delivered from a scratch
    /// buffer (entity decode, normalization). Consumers walk `extend_to_paired`
    /// to recover spans for the `None` case.
    pub source: Option<SourceSpan>,
    /// Semantic role of this grapheme. Used by source-mode copy to
    /// dispatch per-construct extraction rules.
    pub source_kind: SourceKind,
    /// `Some(kind)` for cells the renderer emitted without source backing
    /// (bullets, blockquote bars, table borders, heading markers, etc.).
    /// `None` for content cells.
    pub decorative: Option<DecorativeKind>,
}

impl CharMapping {
    /// Create a new character mapping (formatting-only; no source info).
    /// Used by callers that don't enable source-position tracking.
    pub fn new(render_offset: usize, formatting: Vec<FormatMark>) -> Self {
        Self {
            render_offset,
            formatting,
            source: None,
            source_kind: SourceKind::PlainText,
            decorative: None,
        }
    }

    /// Create a new character mapping with full source-aware metadata.
    /// Used by callers with source-position tracking enabled.
    pub fn new_kinded(
        render_offset: usize,
        formatting: Vec<FormatMark>,
        source: Option<SourceSpan>,
        source_kind: SourceKind,
        decorative: Option<DecorativeKind>,
    ) -> Self {
        Self {
            render_offset,
            formatting,
            source,
            source_kind,
            decorative,
        }
    }

    /// Returns the nesting depth of formatting.
    #[inline]
    pub fn formatting_depth(&self) -> usize {
        self.formatting.len()
    }

    /// Check if this position has a specific format active.
    #[inline]
    pub fn has_format(&self, mark: FormatMark) -> bool {
        self.formatting.contains(&mark)
    }
}

/// Position mappings for a single rendered line.
#[derive(Debug, Clone, Default)]
pub struct LinePosMap {
    /// Character mappings, sorted by render_offset.
    chars: Vec<CharMapping>,
}

impl LinePosMap {
    /// Create a new empty line position map.
    pub fn new() -> Self {
        Self { chars: Vec::new() }
    }

    /// Create with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            chars: Vec::with_capacity(capacity),
        }
    }

    /// Add a character mapping. Must be called in ascending render_offset
    /// order — `LinePosMap::mapping_at` uses binary search and requires
    /// the entries to be sorted.
    ///
    /// Two kinds of callers exist. The renderer's source-aware path
    /// (`push_text_position_mappings` + `push_decorative_position_mappings`)
    /// pushes consecutive indices `0, 1, 2, …`; `MarkdownSourceMap::find_mapping`
    /// relies on this contiguity to use `nth(local)` instead of binary
    /// searching, AND to interpret `anchor.grapheme` as a flat index across
    /// lines. Tests and other consumers that exercise the binary-search
    /// floor semantics may push sparse indices (0, 5, 10) — those callers
    /// must use `mapping_at` instead of any flat-index walk.
    ///
    /// The decorative-exclusivity invariant (a grapheme MUST NOT carry both
    /// `Some(source)` and `Some(decorative)`) is enforced here as a
    /// debug_assert.
    #[inline]
    pub fn push(&mut self, mapping: CharMapping) {
        debug_assert!(
            self.chars
                .last()
                .map_or(true, |last| last.render_offset < mapping.render_offset),
            "CharMappings must be pushed in ascending render_offset order"
        );
        debug_assert!(
            !(mapping.source.is_some() && mapping.decorative.is_some()),
            "Decorative exclusivity violated: a grapheme MUST NOT carry \
             both Some(source) and Some(decorative). render_offset={}",
            mapping.render_offset,
        );
        self.chars.push(mapping);
    }

    /// O(log n) lookup: rendered char index → CharMapping.
    ///
    /// Returns the mapping at or before the given offset (floor semantics).
    pub fn mapping_at(&self, render_offset: usize) -> Option<&CharMapping> {
        if self.chars.is_empty() {
            return None;
        }

        match self
            .chars
            .binary_search_by_key(&render_offset, |c| c.render_offset)
        {
            Ok(i) => Some(&self.chars[i]),
            Err(i) if i > 0 => Some(&self.chars[i - 1]),
            Err(_) => None,
        }
    }

    /// Get the formatting stack at a given render offset.
    pub fn formatting_at(&self, render_offset: usize) -> Option<&[FormatMark]> {
        self.mapping_at(render_offset)
            .map(|m| m.formatting.as_slice())
    }

    /// Number of mapped characters.
    #[inline]
    pub fn len(&self) -> usize {
        self.chars.len()
    }

    /// Whether this map is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Iterate over all character mappings.
    pub fn iter(&self) -> impl Iterator<Item = &CharMapping> {
        self.chars.iter()
    }

    /// Consume this line and return owned `CharMapping`s. Used by the wrap
    /// path: the pre-wrap line's mappings are distributed across the
    /// per-visual-line `LinePosMap`s the wrap engine builds.
    pub fn into_chars(self) -> Vec<CharMapping> {
        self.chars
    }

    /// Prepend `count` decorative `CharMapping` entries to this line and
    /// shift all existing entries' `render_offset` up by `count`. Used by
    /// the renderer when a per-line prefix (blockquote bar, etc.) is added
    /// AFTER content has been pushed — keeps position_map's per-line
    /// grapheme indices aligned with the actual rendered cell columns.
    pub fn prepend_decoratives(
        &mut self,
        count: usize,
        kind: DecorativeKind,
        source_kind: SourceKind,
    ) {
        if count == 0 {
            return;
        }
        for ch in self.chars.iter_mut() {
            ch.render_offset += count;
        }
        let mut new_chars: Vec<CharMapping> = (0..count)
            .map(|i| CharMapping {
                render_offset: i,
                formatting: Vec::new(),
                source: None,
                source_kind,
                decorative: Some(kind),
            })
            .collect();
        new_chars.append(&mut self.chars);
        self.chars = new_chars;
    }
}

/// Position map for an entire rendered markdown document.
///
/// Maps rendered positions back to formatting context for selection
/// and extraction with markdown awareness.
#[derive(Debug, Clone, Default)]
pub struct PositionMap {
    /// Per-line position mappings, indexed by rendered line number.
    lines: Vec<LinePosMap>,
}

impl PositionMap {
    /// Create a new empty position map.
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Create with pre-allocated line capacity.
    pub fn with_capacity(line_count: usize) -> Self {
        Self {
            lines: Vec::with_capacity(line_count),
        }
    }

    /// Start a new line in the position map.
    pub fn start_line(&mut self) {
        self.lines.push(LinePosMap::new());
    }

    /// Start a new line with pre-allocated character capacity.
    pub fn start_line_with_capacity(&mut self, char_capacity: usize) {
        self.lines.push(LinePosMap::with_capacity(char_capacity));
    }

    /// Get the current (last) line being built, if any.
    pub fn current_line_mut(&mut self) -> Option<&mut LinePosMap> {
        self.lines.last_mut()
    }

    /// Get position map for a specific line.
    pub fn line(&self, line_idx: usize) -> Option<&LinePosMap> {
        self.lines.get(line_idx)
    }

    /// Get mutable position map for a specific line.
    pub fn line_mut(&mut self, line_idx: usize) -> Option<&mut LinePosMap> {
        self.lines.get_mut(line_idx)
    }

    /// Pop the last line from the map (used to trim trailing empties at
    /// end of render so `line_count()` mirrors `text.lines.len()`).
    pub fn pop_last_line(&mut self) -> Option<LinePosMap> {
        self.lines.pop()
    }

    /// Replace the current (last) line with the given lines.
    ///
    /// Used by the wrap path: a single logical line that got wrapped into
    /// K visual lines needs its position_map representation re-shaped to
    /// match — same K, each with its own per-line `render_offset` indexing
    /// starting at 0.
    ///
    /// If the map is empty (no current line), this acts as a plain extend.
    pub fn replace_current_with(&mut self, new_lines: Vec<LinePosMap>) {
        if !self.lines.is_empty() {
            self.lines.pop();
        }
        self.lines.extend(new_lines);
    }

    /// Get the formatting at a specific (line, char) position.
    pub fn formatting_at(&self, line_idx: usize, render_offset: usize) -> Option<&[FormatMark]> {
        self.line(line_idx)?.formatting_at(render_offset)
    }

    /// Number of lines in the position map.
    #[inline]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Whether this map is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Iterate over all line position maps.
    pub fn iter(&self) -> impl Iterator<Item = &LinePosMap> {
        self.lines.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_mark_syntax() {
        assert_eq!(FormatMark::Bold.open_syntax(), "**");
        assert_eq!(FormatMark::Bold.close_syntax(), "**");
        assert_eq!(FormatMark::Italic.open_syntax(), "_");
        assert_eq!(FormatMark::Code.open_syntax(), "`");
    }

    #[test]
    fn test_char_mapping() {
        let stack = vec![FormatMark::Bold, FormatMark::Italic];
        let mapping = CharMapping::new(5, stack);

        assert_eq!(mapping.render_offset, 5);
        assert_eq!(mapping.formatting_depth(), 2);
        assert!(mapping.has_format(FormatMark::Bold));
        assert!(mapping.has_format(FormatMark::Italic));
        assert!(!mapping.has_format(FormatMark::Code));
    }

    #[test]
    fn test_line_pos_map_lookup() {
        let mut line = LinePosMap::new();

        // Add mappings at positions 0, 5, 10
        line.push(CharMapping::new(0, vec![]));
        line.push(CharMapping::new(5, vec![FormatMark::Bold]));
        line.push(CharMapping::new(
            10,
            vec![FormatMark::Bold, FormatMark::Italic],
        ));

        // Exact matches
        assert_eq!(line.mapping_at(0).unwrap().render_offset, 0);
        assert_eq!(line.mapping_at(5).unwrap().render_offset, 5);
        assert_eq!(line.mapping_at(10).unwrap().render_offset, 10);

        // Floor semantics: between mappings returns previous
        assert_eq!(line.mapping_at(3).unwrap().render_offset, 0);
        assert_eq!(line.mapping_at(7).unwrap().render_offset, 5);
        assert_eq!(line.mapping_at(15).unwrap().render_offset, 10);
    }

    #[test]
    fn test_position_map_building() {
        let mut map = PositionMap::new();

        // Build line 0
        map.start_line();
        map.current_line_mut()
            .unwrap()
            .push(CharMapping::new(0, vec![]));
        map.current_line_mut()
            .unwrap()
            .push(CharMapping::new(5, vec![FormatMark::Bold]));

        // Build line 1
        map.start_line();
        map.current_line_mut()
            .unwrap()
            .push(CharMapping::new(0, vec![FormatMark::Italic]));

        assert_eq!(map.line_count(), 2);
        assert_eq!(map.line(0).unwrap().len(), 2);
        assert_eq!(map.line(1).unwrap().len(), 1);

        // Check formatting lookups
        let fmt = map.formatting_at(0, 5).unwrap();
        assert!(fmt.contains(&FormatMark::Bold));

        let fmt = map.formatting_at(1, 0).unwrap();
        assert!(fmt.contains(&FormatMark::Italic));
    }
}

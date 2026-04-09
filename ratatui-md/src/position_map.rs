//! Position mapping for rendered markdown.
//!
//! Maps rendered character positions back to source context,
//! enabling text selection and extraction with formatting awareness.

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

/// Maps a rendered character position to its formatting context.
#[derive(Debug, Clone)]
pub struct CharMapping {
    /// Grapheme index in the rendered line (0-based).
    pub render_offset: usize,
    /// Active formatting at this character position.
    /// Stack order: outermost to innermost (e.g., [Bold, Italic] for `**_text_**`).
    pub formatting: Vec<FormatMark>,
}

impl CharMapping {
    /// Create a new character mapping.
    pub fn new(render_offset: usize, formatting: Vec<FormatMark>) -> Self {
        Self {
            render_offset,
            formatting,
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

    /// Add a character mapping. Must be called in order (render_offset ascending).
    #[inline]
    pub fn push(&mut self, mapping: CharMapping) {
        debug_assert!(
            self.chars.last().map_or(true, |last| last.render_offset < mapping.render_offset),
            "CharMappings must be pushed in ascending render_offset order"
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

        match self.chars.binary_search_by_key(&render_offset, |c| c.render_offset) {
            Ok(i) => Some(&self.chars[i]),
            Err(i) if i > 0 => Some(&self.chars[i - 1]),
            Err(_) => None,
        }
    }

    /// Get the formatting stack at a given render offset.
    pub fn formatting_at(&self, render_offset: usize) -> Option<&[FormatMark]> {
        self.mapping_at(render_offset).map(|m| m.formatting.as_slice())
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
        line.push(CharMapping::new(10, vec![FormatMark::Bold, FormatMark::Italic]));

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
        map.current_line_mut().unwrap().push(CharMapping::new(0, vec![]));
        map.current_line_mut().unwrap().push(CharMapping::new(5, vec![FormatMark::Bold]));

        // Build line 1
        map.start_line();
        map.current_line_mut().unwrap().push(CharMapping::new(0, vec![FormatMark::Italic]));

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

//! Privacy-projection trait.
//!
//! The renderer applies privacy projection BEFORE parsing markdown, so
//! the `MarkdownSourceMap` is built against the *projected* source
//! string. This means source-mode copy slicing always indexes into
//! projected bytes, never raw `TextBlock.text` — closing the
//! redaction-bypass channel that would otherwise exist if projection
//! were applied as a post-slice transformation (length-changing
//! redactions would shift byte offsets).
//!
//! Cadenza implements `PrivacyProjection` against its `RulesetRegistry`
//! and passes the impl into `render_with_block_and_privacy`. ratatui-md
//! doesn't know anything about the actual redaction rules.
//!
//! Implementations should return `Cow::Borrowed(input)` when no
//! redaction applies — avoids an allocation in the common case
//! (`PrivacyMode::Reveal`).
//!
//! Documented limitation (see plan §I.1.6 fact 2): redaction rules that
//! insert markdown-significant characters inside a markdown construct
//! (e.g., `[Contact: [EMAIL]](url)`) may produce projected source that
//! isn't valid markdown. The renderer parses what it's given;
//! malformations are visible to the user in their copied output.
//! Cadenza warns once-per-session on first source-copy in active
//! redaction mode.

use std::borrow::Cow;

/// A privacy projection from a source string to its rendered/copyable
/// projected form. Implementations are typically backed by a
/// `RulesetRegistry` or similar redaction engine in the consumer.
pub trait PrivacyProjection: Send + Sync {
    /// Project `source` to its privacy-redacted form.
    ///
    /// Returns `Cow::Borrowed(source)` when no redaction applies
    /// (avoids allocation in the common case). Returns `Cow::Owned(_)`
    /// when redaction modifies the source. The returned string is
    /// what the renderer will parse and what `MarkdownSourceMap::source()`
    /// will return.
    fn project<'a>(&self, source: &'a str) -> Cow<'a, str>;
}

/// A test-friendly projection that's the identity — for unit tests and
/// the `PrivacyMode::Reveal` case where Cadenza wants to call through
/// the projection-bearing API without actually masking anything.
#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityProjection;

impl PrivacyProjection for IdentityProjection {
    fn project<'a>(&self, source: &'a str) -> Cow<'a, str> {
        Cow::Borrowed(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_projection_returns_borrowed() {
        let p = IdentityProjection;
        let s = "hello world";
        let out = p.project(s);
        match out {
            Cow::Borrowed(b) => assert_eq!(b, s),
            Cow::Owned(_) => panic!("IdentityProjection should always return Borrowed"),
        }
    }
}

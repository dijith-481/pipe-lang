/// A span representing a range of bytes in the source code.
///
/// Spans are used throughout the compiler for error reporting, LSP support,
/// and JIT debugging. All indices are byte offsets (not char offsets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Creates a new span from start and end byte offsets.
    #[must_use]
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Creates a zero-width span at the given byte offset.
    #[must_use]
    pub fn empty(pos: usize) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }

    /// Merges two spans into one that covers both ranges.
    #[must_use]
    pub fn merge(self, other: Span) -> Span {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Returns the length of the span in bytes.
    #[must_use]
    pub fn len(self) -> usize {
        self.end - self.start
    }

    /// Returns true if the span covers zero bytes.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Extracts the source text for this span from the given source string.
    ///
    /// # Panics
    ///
    /// Panics if the span is out of bounds for the source string.
    #[must_use]
    pub fn source_text(self, source: &str) -> &str {
        &source[self.start..self.end]
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_new_stores_offsets() {
        let span = Span::new(5, 10);
        assert_eq!(span.start, 5);
        assert_eq!(span.end, 10);
    }

    #[test]
    fn span_empty_has_zero_length() {
        let span = Span::empty(7);
        assert_eq!(span.start, 7);
        assert_eq!(span.end, 7);
        assert!(span.is_empty());
        assert_eq!(span.len(), 0);
    }

    #[test]
    fn span_merge_takes_widest_range() {
        let a = Span::new(2, 8);
        let b = Span::new(5, 12);
        let merged = a.merge(b);
        assert_eq!(merged, Span::new(2, 12));
    }

    #[test]
    fn span_merge_order_independent() {
        let a = Span::new(5, 12);
        let b = Span::new(2, 8);
        assert_eq!(a.merge(b), b.merge(a));
    }

    #[test]
    fn span_len_computes_byte_difference() {
        let span = Span::new(0, 42);
        assert_eq!(span.len(), 42);
    }

    #[test]
    fn span_display_shows_range() {
        let span = Span::new(10, 20);
        assert_eq!(format!("{span}"), "10..20");
    }

    #[test]
    fn span_source_text_extracts_slice() {
        let source = "hello world";
        let span = Span::new(6, 11);
        assert_eq!(span.source_text(source), "world");
    }

    #[test]
    fn span_is_copy() {
        let a = Span::new(0, 5);
        let b = a;
        assert_eq!(a, b);
    }
}

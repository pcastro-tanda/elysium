use std::ops::Range;

/// A half-open byte range `[start, end)` into a source file.
///
/// Offsets are `u32`: a single Ruby file over 4 GiB is not a supported input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Span {
    /// Start byte offset, inclusive.
    pub start: u32,
    /// End byte offset, exclusive.
    pub end: u32,
}

impl Span {
    /// Creates a span. `start` must not exceed `end`.
    pub const fn new(start: u32, end: u32) -> Self {
        debug_assert!(start <= end);
        Self { start, end }
    }

    /// Creates a span from `usize` offsets, saturating at `u32::MAX`.
    pub fn from_usize(start: usize, end: usize) -> Self {
        Self::new(u32::try_from(start).unwrap_or(u32::MAX), u32::try_from(end).unwrap_or(u32::MAX))
    }

    /// Zero-width span at `offset`.
    pub const fn empty(offset: u32) -> Self {
        Self { start: offset, end: offset }
    }

    /// Length in bytes.
    pub const fn len(&self) -> u32 {
        self.end - self.start
    }

    /// True when the span covers no bytes.
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// The span as a `usize` range for slicing.
    pub fn range(&self) -> Range<usize> {
        self.start as usize..self.end as usize
    }

    /// True when `other` shares at least one byte with `self`, or when either
    /// is empty and sits strictly inside the other.
    pub const fn overlaps(&self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// True when `self` fully contains `other`.
    pub const fn contains(&self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// Smallest span covering both.
    #[must_use]
    pub fn join(&self, other: Self) -> Self {
        Self::new(self.start.min(other.start), self.end.max(other.end))
    }
}

impl From<Range<usize>> for Span {
    fn from(r: Range<usize>) -> Self {
        Self::from_usize(r.start, r.end)
    }
}

//! Source file model shared by every other crate.
//!
//! A [`SourceFile`] owns the raw bytes of one Ruby file plus a [`LineIndex`]
//! that maps byte offsets to line/column pairs. Everything downstream (the
//! parser facade, diagnostics, fixes) talks in byte offsets; only the
//! formatters convert to lines and columns at the very end.
//!
//! Bytes are kept as read from disk. Ruby sources are almost always UTF-8, but
//! a `# encoding:` magic comment can legally introduce other encodings, and a
//! lossy conversion would shift byte offsets and corrupt fixes. Column
//! computation therefore counts UTF-8 scalar starts and treats any invalid
//! byte as one column, which matches what RuboCop reports for valid files and
//! degrades gracefully for invalid ones.

use std::fmt;
use std::path::{Path, PathBuf};

mod line_index;
mod span;

pub use line_index::{LineCol, LineIndex};
pub use span::Span;

/// One Ruby source file held in memory.
#[derive(Debug)]
pub struct SourceFile {
    path: PathBuf,
    bytes: Box<[u8]>,
    lines: LineIndex,
}

impl SourceFile {
    /// Builds a source file from an in-memory buffer.
    pub fn new(path: impl Into<PathBuf>, bytes: impl Into<Box<[u8]>>) -> Self {
        let bytes = bytes.into();
        let lines = LineIndex::new(&bytes);
        Self { path: path.into(), bytes, lines }
    }

    /// Reads a source file from disk.
    pub fn read(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        let bytes = std::fs::read(&path)?;
        Ok(Self::new(path, bytes))
    }

    /// Path this file was loaded from (or was given at construction).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Raw bytes of the file.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The file contents as `&str` if they are valid UTF-8.
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes).ok()
    }

    /// The line index for this file.
    pub fn lines(&self) -> &LineIndex {
        &self.lines
    }

    /// Bytes covered by `span`.
    ///
    /// # Panics
    /// Panics if `span` is out of bounds.
    pub fn slice(&self, span: Span) -> &[u8] {
        &self.bytes[span.range()]
    }

    /// Line and column of a byte offset. See [`LineIndex::line_col`].
    pub fn line_col(&self, offset: u32) -> LineCol {
        self.lines.line_col(&self.bytes, offset)
    }

    /// Text of a 1-based line, without its trailing line terminator.
    pub fn line_text(&self, line: u32) -> &[u8] {
        self.lines.line_text(&self.bytes, line)
    }

    /// Number of lines. An empty file has one (empty) line; a file ending in
    /// a newline does not count an extra empty line, matching RuboCop.
    pub fn line_count(&self) -> u32 {
        self.lines.line_count(&self.bytes)
    }
}

impl fmt::Display for SourceFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

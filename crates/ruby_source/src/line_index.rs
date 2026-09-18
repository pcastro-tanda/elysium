/// Line and column of a byte offset.
///
/// `line` is 1-based. `column` is 0-based and counted in characters (UTF-8
/// scalar values), which is what RuboCop's `Parser::Source` reports
/// internally; formatters add one where RuboCop's output does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LineCol {
    /// 1-based line number.
    pub line: u32,
    /// 0-based character column.
    pub column: u32,
}

/// Byte offsets of every line start, for offset to line/column mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    /// `starts[i]` is the byte offset where 1-based line `i + 1` begins.
    /// `starts[0]` is always 0.
    starts: Vec<u32>,
}

impl LineIndex {
    /// Scans `bytes` for `\n` and records each line start.
    pub fn new(bytes: &[u8]) -> Self {
        let mut starts = Vec::with_capacity(bytes.len() / 32 + 1);
        starts.push(0);
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                starts.push(u32::try_from(i + 1).expect("file exceeds u32 offsets"));
            }
        }
        Self { starts }
    }

    /// 1-based line containing `offset`. An offset equal to the file length
    /// maps to the last line.
    pub fn line(&self, offset: u32) -> u32 {
        let idx = match self.starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        u32::try_from(idx + 1).expect("line count exceeds u32")
    }

    /// Byte offset of the start of a 1-based line.
    ///
    /// # Panics
    /// Panics if `line` is 0 or beyond the last line.
    pub fn line_start(&self, line: u32) -> u32 {
        self.starts[line as usize - 1]
    }

    /// Line and character column of a byte offset.
    pub fn line_col(&self, bytes: &[u8], offset: u32) -> LineCol {
        let line = self.line(offset);
        let start = self.line_start(line);
        let prefix = &bytes[start as usize..(offset as usize).min(bytes.len())];
        // Every UTF-8 scalar has exactly one byte that is not a continuation
        // byte (0b10xxxxxx). Invalid bytes are counted as a column each.
        let column = prefix.iter().filter(|&&b| (b & 0xC0) != 0x80).count();
        LineCol { line, column: u32::try_from(column).expect("column exceeds u32") }
    }

    /// Text of a 1-based line, without `\n` or `\r\n`.
    pub fn line_text<'a>(&self, bytes: &'a [u8], line: u32) -> &'a [u8] {
        let start = self.line_start(line) as usize;
        let end = self.starts.get(line as usize).map_or(bytes.len(), |&s| s as usize);
        let mut text = &bytes[start..end];
        if let [rest @ .., b'\n'] = text {
            text = rest;
        }
        if let [rest @ .., b'\r'] = text {
            text = rest;
        }
        text
    }

    /// Number of lines, not counting a trailing empty line after a final `\n`.
    pub fn line_count(&self, bytes: &[u8]) -> u32 {
        let n = self.starts.len();
        let count = if n > 1 && bytes.last() == Some(&b'\n') { n - 1 } else { n };
        u32::try_from(count).expect("line count exceeds u32")
    }

    /// Byte offsets where each line starts, in ascending order.
    pub fn line_starts(&self) -> &[u32] {
        &self.starts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lc(line: u32, column: u32) -> LineCol {
        LineCol { line, column }
    }

    #[test]
    fn maps_offsets_to_lines_and_columns() {
        let src = b"ab\ncd\n\nefg";
        let idx = LineIndex::new(src);
        assert_eq!(idx.line_col(src, 0), lc(1, 0));
        assert_eq!(idx.line_col(src, 2), lc(1, 2));
        assert_eq!(idx.line_col(src, 3), lc(2, 0));
        assert_eq!(idx.line_col(src, 6), lc(3, 0));
        assert_eq!(idx.line_col(src, 7), lc(4, 0));
        assert_eq!(idx.line_col(src, 10), lc(4, 3));
    }

    #[test]
    fn columns_count_characters_not_bytes() {
        let src = "x = \"héllo\" # ü".as_bytes();
        let idx = LineIndex::new(src);
        let after_e_acute = 4 + 1 + 1 + 2; // x,space,=,space,",h,é(2 bytes)
        assert_eq!(idx.line_col(src, after_e_acute), lc(1, 7));
        assert_eq!(idx.line_col(src, u32::try_from(src.len()).unwrap()), lc(1, 15));
    }

    #[test]
    fn invalid_utf8_counts_one_column_per_byte() {
        let src = b"a\xff\xfeb";
        let idx = LineIndex::new(src);
        assert_eq!(idx.line_col(src, 4), lc(1, 4));
    }

    #[test]
    fn line_text_strips_terminators() {
        let src = b"one\r\ntwo\nthree";
        let idx = LineIndex::new(src);
        assert_eq!(idx.line_text(src, 1), b"one");
        assert_eq!(idx.line_text(src, 2), b"two");
        assert_eq!(idx.line_text(src, 3), b"three");
    }

    #[test]
    fn line_count_matches_rubocop() {
        assert_eq!(LineIndex::new(b"").line_count(b""), 1);
        assert_eq!(LineIndex::new(b"a").line_count(b"a"), 1);
        assert_eq!(LineIndex::new(b"a\n").line_count(b"a\n"), 1);
        assert_eq!(LineIndex::new(b"a\nb").line_count(b"a\nb"), 2);
        assert_eq!(LineIndex::new(b"a\n\n").line_count(b"a\n\n"), 2);
    }
}

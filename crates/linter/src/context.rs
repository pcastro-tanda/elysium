use std::borrow::Cow;
use std::cell::OnceCell;

use ruby_ast::{LocationExt as _, NodeKind, Parsed};
use ruby_directives::Directives;
use ruby_source::{LineCol, SourceFile, Span};

use crate::diagnostic::{Diagnostic, Fix};
use crate::rule::RuleMeta;

/// Kind and span of one node on the ancestor stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeInfo {
    /// The node's kind.
    pub kind: NodeKind,
    /// The node's byte span.
    pub span: Span,
}

/// One comment in the file, in source order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentInfo {
    /// Byte range of the comment, including the leading `#`.
    pub span: Span,
    /// 1-based line the comment starts on.
    pub line: u32,
}

/// Per-file state handed to rules: the source, the tree, the directive
/// comments, and the diagnostic sink. Designed so every method could be
/// marshalled over a WASM boundary later: inputs are plain offsets and
/// strings, outputs are values.
pub struct Context<'a> {
    source: &'a SourceFile,
    parsed: &'a Parsed<'a>,
    directives: Directives,
    diagnostics: Vec<Diagnostic>,
    comments: OnceCell<Vec<CommentInfo>>,
    ancestors: Vec<NodeInfo>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(
        source: &'a SourceFile,
        parsed: &'a Parsed<'a>,
        directives: Directives,
    ) -> Self {
        Self {
            source,
            parsed,
            directives,
            diagnostics: Vec::new(),
            comments: OnceCell::new(),
            ancestors: Vec::with_capacity(64),
        }
    }

    /// The file being linted.
    pub fn source(&self) -> &'a SourceFile {
        self.source
    }

    /// The parsed tree (for comments, magic comments, `__END__`).
    pub fn parsed(&self) -> &'a Parsed<'a> {
        self.parsed
    }

    /// The `# rubocop:disable`/`enable`/`todo` directive comments found in
    /// this file. Empty when the file had syntax errors (nothing but
    /// [`crate::SYNTAX_RULE`] is produced for those, and it is never
    /// suppressible).
    pub fn directives(&self) -> &Directives {
        &self.directives
    }

    /// Source bytes covered by `span`.
    pub fn text(&self, span: Span) -> &'a [u8] {
        self.source.slice(span)
    }

    /// Line and column of a byte offset.
    pub fn line_col(&self, offset: u32) -> LineCol {
        self.source.line_col(offset)
    }

    /// Text of a 1-based line, without its line terminator.
    pub fn line_text(&self, line: u32) -> &'a [u8] {
        self.source.line_text(line)
    }

    /// Byte range of a 1-based line, without its line terminator.
    pub fn line_span(&self, line: u32) -> Span {
        let start = self.source.lines().line_start(line);
        let len = u32::try_from(self.source.line_text(line).len()).unwrap_or(u32::MAX);
        Span::new(start, start + len)
    }

    /// Number of lines in the file.
    pub fn line_count(&self) -> u32 {
        self.source.line_count()
    }

    /// Every line's 1-based number and its byte span, without a trailing
    /// line terminator. Built directly from the line index; no allocation.
    pub fn lines(&self) -> impl Iterator<Item = (u32, Span)> + 'a {
        let bytes = self.source.bytes();
        let starts = self.source.lines().line_starts();
        let count = self.source.line_count();
        (0..count).map(move |i| {
            let idx = i as usize;
            let start = starts[idx];
            let mut end = starts
                .get(idx + 1)
                .copied()
                .unwrap_or_else(|| u32::try_from(bytes.len()).unwrap_or(u32::MAX));
            if end > start && bytes[(end - 1) as usize] == b'\n' {
                end -= 1;
                if end > start && bytes[(end - 1) as usize] == b'\r' {
                    end -= 1;
                }
            }
            (i + 1, Span::new(start, end))
        })
    }

    /// Ancestors of the node currently being entered or left, outermost
    /// first. Excludes the node itself.
    pub fn ancestors(&self) -> &[NodeInfo] {
        &self.ancestors
    }

    /// The immediate parent of the node currently being entered or left, if
    /// any.
    pub fn parent(&self) -> Option<NodeInfo> {
        self.ancestors.last().copied()
    }

    /// Nesting depth of the node currently being entered or left: the
    /// number of ancestors above it (`0` at the root).
    pub fn depth(&self) -> usize {
        self.ancestors.len()
    }

    /// Pushes the node just entered onto the ancestor stack, for the
    /// benefit of its descendants.
    pub(crate) fn push_ancestor(&mut self, info: NodeInfo) {
        self.ancestors.push(info);
    }

    /// Pops the node about to be left off the ancestor stack.
    pub(crate) fn pop_ancestor(&mut self) {
        self.ancestors.pop();
    }

    /// Every comment in the file, in source order. Built once per file.
    pub fn comments(&self) -> &[CommentInfo] {
        self.comments.get_or_init(|| {
            self.parsed
                .comments()
                .map(|comment| {
                    let span = comment.location().span();
                    CommentInfo { span, line: self.source.line_col(span.start).line }
                })
                .collect()
        })
    }

    /// Reports an offense at `span` with the rule's default severity.
    pub fn report(&mut self, rule: &RuleMeta, span: Span, message: impl Into<Cow<'static, str>>) {
        self.diagnostics.push(Diagnostic::new(rule.name, span, rule.severity, message));
    }

    /// Reports an offense with an attached fix.
    pub fn report_with_fix(
        &mut self,
        rule: &RuleMeta,
        span: Span,
        message: impl Into<Cow<'static, str>>,
        fix: Fix,
    ) {
        self.diagnostics
            .push(Diagnostic::new(rule.name, span, rule.severity, message).with_fix(fix));
    }

    /// Pushes a fully built diagnostic.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub(crate) fn into_parts(self) -> (Vec<Diagnostic>, Directives) {
        (self.diagnostics, self.directives)
    }
}

#[cfg(test)]
mod tests {
    use ruby_ast::Parsed;
    use ruby_directives::Directives;
    use ruby_source::SourceFile;

    use super::*;

    #[test]
    fn lines_matches_line_text_for_crlf_file() {
        let source = SourceFile::new("a.rb", b"foo\r\nbar\r\nbaz".to_vec());
        let parsed = Parsed::parse(&source);
        let ctx = Context::new(&source, &parsed, Directives::default());
        let lines: Vec<_> = ctx.lines().collect();
        assert_eq!(lines.len(), source.line_count() as usize);
        for (line, span) in lines {
            assert_eq!(source.slice(span), source.line_text(line));
        }
    }

    #[test]
    fn lines_matches_line_text_for_no_trailing_newline_file() {
        let source = SourceFile::new("a.rb", b"one\ntwo\nthree".to_vec());
        let parsed = Parsed::parse(&source);
        let ctx = Context::new(&source, &parsed, Directives::default());
        let lines: Vec<_> = ctx.lines().collect();
        assert_eq!(lines.len(), 3);
        for (line, span) in lines {
            assert_eq!(source.slice(span), source.line_text(line));
        }
    }
}

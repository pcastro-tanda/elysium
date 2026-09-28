use std::borrow::Cow;
use std::cell::OnceCell;

use ruby_ast::{
    each_descendant, CommentType, LocationExt as _, Node, NodeExt as _, NodeKind, Parsed,
};
use ruby_directives::Directives;
use ruby_semantic::Semantics;
use ruby_source::{LineCol, Side, SourceFile, Span};

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

/// Which comment syntax a [`CommentInfo`] came from -- Prism's
/// `CommentType`, mirroring RuboCop's `Parser::Source::Comment#type`
/// (`:inline` / `:document`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    /// A `#`-prefixed comment, to the end of its line.
    Inline,
    /// A `=begin`/`=end` block comment.
    EmbDoc,
}

/// One comment in the file, in source order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentInfo {
    /// Byte range of the comment, including the leading `#` (or the whole
    /// `=begin`..`=end` block for [`CommentKind::EmbDoc`]).
    pub span: Span,
    /// 1-based line the comment starts on.
    pub line: u32,
    /// Which comment syntax this is.
    pub kind: CommentKind,
}

/// The span a whole-file offense carries: zero length, at byte 0. RuboCop's
/// `Offense::NO_LOCATION` (`PseudoSourceRange.new(1, 0, '', 0, 0)`), the
/// range `Base#add_global_offense` reports at. See
/// [`Context::report_global`].
pub const GLOBAL_SPAN: Span = Span { start: 0, end: 0 };

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
    opaque_spans: OnceCell<Vec<Span>>,
    semantics: OnceCell<Semantics<'a>>,
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
            opaque_spans: OnceCell::new(),
            semantics: OnceCell::new(),
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

    /// RuboCop's `Alignment#display_column`: the rendered display width of
    /// the text preceding `offset` on its own line. See
    /// [`SourceFile::display_column`].
    pub fn display_column(&self, offset: u32) -> u32 {
        self.source.display_column(offset)
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

    /// `rubocop-ast`'s `Node#single_line?`. See [`SourceFile::is_single_line`].
    pub fn is_single_line(&self, span: Span) -> bool {
        self.source.is_single_line(span)
    }

    /// RuboCop's `Range#last_line`. See [`SourceFile::last_line`].
    pub fn last_line(&self, span: Span) -> u32 {
        self.source.last_line(span)
    }

    /// True when `a` and `b` start on the same source line. See
    /// [`SourceFile::same_line`].
    pub fn same_line(&self, a: Span, b: Span) -> bool {
        self.source.same_line(a, b)
    }

    /// RuboCop's `Util#begins_its_line?`. See [`SourceFile::begins_its_line`].
    pub fn begins_its_line(&self, span: Span) -> bool {
        self.source.begins_its_line(span)
    }

    /// `rubocop-ast`'s `RangeHelp#range_by_whole_lines`. See
    /// [`SourceFile::whole_lines`].
    pub fn whole_lines(&self, span: Span) -> Span {
        self.source.whole_lines(span)
    }

    /// `rubocop-ast`'s `RangeHelp#range_with_surrounding_space`. See
    /// [`SourceFile::with_surrounding_space`].
    pub fn with_surrounding_space(
        &self,
        span: Span,
        side: Side,
        newlines: bool,
        whitespace: bool,
    ) -> Span {
        self.source.with_surrounding_space(span, side, newlines, whitespace)
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
                    let kind = match comment.type_() {
                        CommentType::EmbDocComment => CommentKind::EmbDoc,
                        CommentType::InlineComment => CommentKind::Inline,
                    };
                    CommentInfo { span, line: self.source.line_col(span.start).line, kind }
                })
                .collect()
        })
    }

    /// Byte ranges a raw-text scan must not read as code, sorted by start
    /// offset and merged so they never overlap: the *bodies* of string,
    /// `%x`/backtick, symbol and regexp literals (heredoc bodies included,
    /// since Prism models a heredoc as a string node whose content location
    /// is the body), plus every comment.
    ///
    /// There is no single RuboCop equivalent: upstream has a real token
    /// stream and so never sees these bytes as code in the first place
    /// (`ProcessedSource#tokens` simply has no token inside a string body or
    /// a comment). This is the shared approximation raw-byte-scanning rules
    /// use instead of each hand-rolling its own string/comment skipping
    /// (`Style/Semicolon`, `Style/LineEndConcatenation`,
    /// `Style/CommentedKeyword` and friends).
    ///
    /// An interpolated literal contributes only its literal parts: the code
    /// inside `#{...}` is real code and is deliberately left out, so an
    /// offset inside an interpolation is not opaque -- except where it is
    /// itself inside a literal nested in that interpolation.
    ///
    /// Built once per file on first use, like [`Context::comments`] and
    /// [`Context::semantics`]; a file whose enabled rules never ask pays
    /// nothing.
    pub fn opaque_spans(&self) -> &[Span] {
        self.opaque_spans.get_or_init(|| {
            let mut spans: Vec<Span> = self.comments().iter().map(|c| c.span).collect();
            let root = self.parsed.root();
            let mut collect = |node: &Node<'_>| push_literal_body(&mut spans, node);
            collect(&root);
            each_descendant(&root, &mut collect);
            spans.retain(|span| span.end > span.start);
            spans.sort_by_key(|span| (span.start, span.end));
            // Literals nest (a string inside an interpolation inside a
            // heredoc), so merge before handing the list out: callers then
            // get one lookup per offset instead of a walk back over every
            // possibly enclosing span.
            let mut merged: Vec<Span> = Vec::with_capacity(spans.len());
            for span in spans {
                match merged.last_mut() {
                    Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
                    _ => merged.push(span),
                }
            }
            merged
        })
    }

    /// True when `offset` falls inside a span from [`Context::opaque_spans`]
    /// (start-inclusive, end-exclusive). Binary search over the merged list.
    pub fn in_opaque_span(&self, offset: u32) -> bool {
        let spans = self.opaque_spans();
        let upper = spans.partition_point(|span| span.start <= offset);
        upper > 0 && offset < spans[upper - 1].end
    }

    /// Scopes, local variables, assignments and branches for this file,
    /// built on first use by a second traversal of the tree (ADR 0007).
    /// A file whose enabled rules never ask for it pays nothing.
    pub fn semantics(&self) -> &Semantics<'a> {
        self.semantics.get_or_init(|| Semantics::build(&self.parsed.root()))
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

    /// Reports an offense that belongs to the file as a whole rather than to
    /// any node -- RuboCop's `Base#add_global_offense`, which builds its
    /// offense at `Offense::NO_LOCATION` (a `PseudoSourceRange` of line 1,
    /// column 0, length 0).
    ///
    /// The convention here is [`GLOBAL_SPAN`]: the zero-length span at byte
    /// 0. It reports as line 1, column 1 in the CLI's output and as an
    /// `^{}` annotation in the fixture harness, matching how upstream's
    /// `expect_offense` annotates a global offense (see
    /// `spec/rubocop/cop/naming/file_name_spec.rb`), and it sorts before
    /// every located offense.
    pub fn report_global(&mut self, rule: &RuleMeta, message: impl Into<Cow<'static, str>>) {
        self.report(rule, GLOBAL_SPAN, message);
    }

    /// [`Context::report_global`] with an attached fix, for a whole-file
    /// offense whose correction edits somewhere other than the (empty)
    /// offense span -- RuboCop's `add_global_offense` with a corrector
    /// block.
    pub fn report_global_with_fix(
        &mut self,
        rule: &RuleMeta,
        message: impl Into<Cow<'static, str>>,
        fix: Fix,
    ) {
        self.report_with_fix(rule, GLOBAL_SPAN, message, fix);
    }

    /// Pushes a fully built diagnostic.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Takes every diagnostic reported so far, leaving the sink empty.
    /// Used between [`crate::Rule::file_end`] and [`crate::Rule::file_finish`]
    /// so the hook sees a stable, complete snapshot while still being able
    /// to report more through `self`.
    pub(crate) fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.diagnostics)
    }

    pub(crate) fn into_parts(self) -> (Vec<Diagnostic>, Directives) {
        (self.diagnostics, self.directives)
    }
}

/// Pushes the opaque body of `node`, when it is a literal with one.
///
/// Interpolated literals have no body location of their own -- their
/// literal halves are [`NodeKind::StringNode`] parts, which the walk reaches
/// on its own -- so they need no arm here.
fn push_literal_body(spans: &mut Vec<Span>, node: &Node<'_>) {
    let span = match node.kind() {
        NodeKind::StringNode => node.as_string_node().map(|n| n.content_loc().span()),
        NodeKind::XStringNode => node.as_x_string_node().map(|n| n.content_loc().span()),
        NodeKind::RegularExpressionNode => {
            node.as_regular_expression_node().map(|n| n.content_loc().span())
        }
        NodeKind::MatchLastLineNode => {
            node.as_match_last_line_node().map(|n| n.content_loc().span())
        }
        NodeKind::SymbolNode => node.as_symbol_node().and_then(|n| n.value_loc()).map(|l| l.span()),
        _ => None,
    };
    if let Some(span) = span {
        spans.push(span);
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

    /// Offsets of every byte of `needle`'s single occurrence in `text`.
    fn offsets_of(text: &str, needle: &str) -> Vec<u32> {
        let start = text.find(needle).expect("needle present");
        (start..start + needle.len()).map(|i| u32::try_from(i).expect("small file")).collect()
    }

    #[test]
    fn opaque_spans_cover_literal_bodies_and_comments_but_not_interpolated_code() {
        let text = "x = \"a ; b #{c ; d} e\" # trailing ; here\n:sym_a\n/re ; gx/\n";
        let source = SourceFile::new("a.rb", text.as_bytes().to_vec());
        let parsed = Parsed::parse(&source);
        let ctx = Context::new(&source, &parsed, Directives::default());

        for opaque in ["a ; b ", " e", "trailing ; here", "sym_a", "re ; gx"] {
            for offset in offsets_of(text, opaque) {
                assert!(ctx.in_opaque_span(offset), "{opaque:?} at {offset} should be opaque");
            }
        }
        // The interpolated code, the delimiters around it, and real code are
        // not opaque.
        for code in ["x = ", "#{", "c ; d", "}"] {
            for offset in offsets_of(text, code) {
                assert!(!ctx.in_opaque_span(offset), "{code:?} at {offset} should be code");
            }
        }
        // Sorted, merged, and never empty-ranged.
        let spans = ctx.opaque_spans();
        assert!(spans.windows(2).all(|w| w[0].end < w[1].start));
        assert!(spans.iter().all(|s| s.start < s.end));
    }

    #[test]
    fn opaque_spans_cover_heredoc_bodies_only() {
        let text = "x = <<~SQL\n  select ; now\nSQL\ny = 1\n";
        let source = SourceFile::new("a.rb", text.as_bytes().to_vec());
        let parsed = Parsed::parse(&source);
        let ctx = Context::new(&source, &parsed, Directives::default());
        for offset in offsets_of(text, "select ; now") {
            assert!(ctx.in_opaque_span(offset));
        }
        for offset in offsets_of(text, "<<~SQL").into_iter().chain(offsets_of(text, "y = 1")) {
            assert!(!ctx.in_opaque_span(offset));
        }
    }

    #[test]
    fn report_global_lands_on_line_one_column_one_with_no_length() {
        // An empty file is the motivating case (`Lint/EmptyFile`), and the
        // convention has to hold there too.
        for text in [&b""[..], &b"foo\n"[..]] {
            let source = SourceFile::new("a.rb", text.to_vec());
            let parsed = Parsed::parse(&source);
            let mut ctx = Context::new(&source, &parsed, Directives::default());
            ctx.report_global(&TEST_META, "Empty file detected.");
            let (diagnostics, _) = ctx.into_parts();
            let [diagnostic] = diagnostics.as_slice() else { panic!("one diagnostic") };
            assert_eq!(diagnostic.span, GLOBAL_SPAN);
            // What the CLI and the fixture harness render from: line 1,
            // column 1 (0-based 0) and zero length, so the harness emits the
            // `^{}` annotation upstream's `expect_offense` uses for a global
            // offense.
            let start = source.line_col(diagnostic.span.start);
            let end = source.line_col(diagnostic.span.end);
            assert_eq!((start.line, start.column), (1, 0));
            assert_eq!(end.column - start.column, 0);
        }
    }

    static TEST_META: RuleMeta = RuleMeta {
        name: "Lint/EmptyFile",
        department: crate::rule::Department::Lint,
        summary: "",
        explanation: "",
        enabled_by_default: true,
        severity: crate::diagnostic::Severity::Warning,
        fix: crate::rule::FixAvailability::None,
        stability: crate::rule::Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };
}

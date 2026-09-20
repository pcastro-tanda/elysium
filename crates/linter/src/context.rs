use std::borrow::Cow;

use ruby_ast::Parsed;
use ruby_directives::Directives;
use ruby_source::{SourceFile, Span};

use crate::diagnostic::{Diagnostic, Fix};
use crate::rule::RuleMeta;

/// Per-file state handed to rules: the source, the tree, the directive
/// comments, and the diagnostic sink. Designed so every method could be
/// marshalled over a WASM boundary later: inputs are plain offsets and
/// strings, outputs are values.
pub struct Context<'a> {
    source: &'a SourceFile,
    parsed: &'a Parsed<'a>,
    directives: Directives,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(
        source: &'a SourceFile,
        parsed: &'a Parsed<'a>,
        directives: Directives,
    ) -> Self {
        Self { source, parsed, directives, diagnostics: Vec::new() }
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

use std::borrow::Cow;

use ruby_ast::Parsed;
use ruby_source::{SourceFile, Span};

use crate::diagnostic::{Diagnostic, Fix};
use crate::rule::RuleMeta;

/// Per-file state handed to rules: the source, the tree, and the diagnostic
/// sink. Designed so every method could be marshalled over a WASM boundary
/// later: inputs are plain offsets and strings, outputs are values.
pub struct Context<'a> {
    source: &'a SourceFile,
    parsed: &'a Parsed<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(source: &'a SourceFile, parsed: &'a Parsed<'a>) -> Self {
        Self { source, parsed, diagnostics: Vec::new() }
    }

    /// The file being linted.
    pub fn source(&self) -> &'a SourceFile {
        self.source
    }

    /// The parsed tree (for comments, magic comments, `__END__`).
    pub fn parsed(&self) -> &'a Parsed<'a> {
        self.parsed
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

    pub(crate) fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

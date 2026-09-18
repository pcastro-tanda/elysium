use ruby_ast::{walk, Node, NodeExt, Parsed, Visitor};
use ruby_source::SourceFile;

use crate::context::Context;
use crate::diagnostic::{Diagnostic, Severity};
use crate::rule::Dispatch;

/// Cop name RuboCop uses for parse errors.
pub const SYNTAX_RULE: &str = "Lint/Syntax";

/// Result of linting one file.
#[derive(Debug, Default)]
pub struct FileResult {
    /// Offenses in source order.
    pub diagnostics: Vec<Diagnostic>,
    /// Nodes visited. Zero when the file had syntax errors.
    pub node_count: u32,
    /// True when Prism reported at least one error.
    pub has_syntax_errors: bool,
}

struct Walker<'d, 'c, 'a, D> {
    rules: &'d mut D,
    ctx: &'c mut Context<'a>,
    node_count: u32,
}

impl<'pr, D: Dispatch> Visitor<'pr> for Walker<'_, '_, '_, D> {
    #[inline]
    fn enter(&mut self, node: &Node<'pr>) {
        self.node_count += 1;
        self.rules.enter(node.kind(), node, self.ctx);
    }

    #[inline]
    fn leave(&mut self, node: &Node<'pr>) {
        self.rules.leave(node.kind(), node, self.ctx);
    }
}

/// Parses `source` and runs `rules` over it in a single traversal.
///
/// Mirrors RuboCop: when the file has syntax errors only `Lint/Syntax`
/// offenses are produced and no other rule runs.
pub fn lint_file<D: Dispatch>(source: &SourceFile, rules: &mut D) -> FileResult {
    let parsed = Parsed::parse(source);
    lint_parsed(&parsed, rules)
}

/// Runs `rules` over an already parsed tree. See [`lint_file`].
pub fn lint_parsed<D: Dispatch>(parsed: &Parsed<'_>, rules: &mut D) -> FileResult {
    let source = parsed.source();
    let mut ctx = Context::new(source, parsed);

    let mut has_syntax_errors = false;
    for error in parsed.errors() {
        has_syntax_errors = true;
        ctx.push(Diagnostic::new(SYNTAX_RULE, error.span, Severity::Fatal, error.message));
    }
    if has_syntax_errors {
        return FileResult {
            diagnostics: finish(ctx.into_diagnostics()),
            node_count: 0,
            has_syntax_errors,
        };
    }

    rules.file_start(&mut ctx);
    let node_count = {
        let mut walker = Walker { rules, ctx: &mut ctx, node_count: 0 };
        walk(&parsed.root(), &mut walker);
        walker.node_count
    };
    rules.file_end(&mut ctx);

    FileResult { diagnostics: finish(ctx.into_diagnostics()), node_count, has_syntax_errors }
}

/// Orders diagnostics by position and drops repeats.
///
/// RuboCop's `add_offense` ignores a second offense from the same cop at the
/// same range, so a cop that fires twice on one node (and Prism, which often
/// emits a follow-up "assuming it is closing…" error at the same location)
/// reports once. The sort is stable, so the first-emitted diagnostic wins.
fn finish(mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics.sort_by_key(|d| (d.span.start, d.span.end, d.rule));
    diagnostics.dedup_by(|b, a| a.rule == b.rule && a.span == b.span);
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::NoRules;

    #[test]
    fn clean_file_walks_every_node() {
        let source = SourceFile::new("a.rb", b"puts 1\n".to_vec());
        let result = lint_file(&source, &mut NoRules);
        assert!(result.diagnostics.is_empty());
        assert!(!result.has_syntax_errors);
        // Program, Statements, Call, Arguments, Integer
        assert_eq!(result.node_count, 5);
    }

    #[test]
    fn syntax_error_reports_lint_syntax_and_skips_walk() {
        let source = SourceFile::new("a.rb", b"def foo(\n".to_vec());
        let result = lint_file(&source, &mut NoRules);
        assert!(result.has_syntax_errors);
        assert_eq!(result.node_count, 0);
        assert!(result.diagnostics.iter().all(|d| d.rule == SYNTAX_RULE));
        assert!(result.diagnostics.iter().all(|d| d.severity == Severity::Fatal));
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn same_range_errors_collapse_like_rubocop() {
        // Prism emits two errors at `end`: "expected an expression" and
        // "assuming it is closing the parent method definition". RuboCop
        // reports one offense for this file; so do we.
        let source = SourceFile::new("a.rb", b"def foo(a)\n  a +\nend\n".to_vec());
        let result = lint_file(&source, &mut NoRules);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.starts_with("unexpected 'end'; expected"));
    }
}

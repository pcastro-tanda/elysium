use ruby_ast::{walk, Node, NodeExt, Parsed, Visitor};
use ruby_directives::Directives;
use ruby_source::SourceFile;

use crate::context::{Context, NodeInfo};
use crate::diagnostic::{Diagnostic, Severity};
use crate::rule::Dispatch;
use crate::settings::FileSettings;

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
        let kind = node.kind();
        self.rules.enter(kind, node, self.ctx);
        self.ctx.push_ancestor(NodeInfo { kind, span: node.span() });
    }

    #[inline]
    fn leave(&mut self, node: &Node<'pr>) {
        self.ctx.pop_ancestor();
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

/// Runs `rules` over an already parsed tree with every rule enabled at its
/// default severity. See [`lint_file`].
pub fn lint_parsed<D: Dispatch>(parsed: &Parsed<'_>, rules: &mut D) -> FileResult {
    lint_parsed_with(parsed, rules, &FileSettings::all_enabled())
}

/// Runs `rules` over an already parsed tree, then drops and re-severities
/// diagnostics per `settings` and per any `# rubocop:disable`/`enable`/`todo`
/// directive comments found in the file. See [`lint_file`].
///
/// [`SYNTAX_RULE`] is never dropped or re-severitied: a parse error is
/// always reported as-is, regardless of `settings` or directive comments.
pub fn lint_parsed_with<D: Dispatch>(
    parsed: &Parsed<'_>,
    rules: &mut D,
    settings: &FileSettings,
) -> FileResult {
    let source = parsed.source();

    let mut has_syntax_errors = false;
    let mut syntax_diagnostics = Vec::new();
    for error in parsed.errors() {
        has_syntax_errors = true;
        syntax_diagnostics.push(Diagnostic::new(
            SYNTAX_RULE,
            error.span,
            Severity::Fatal,
            error.message,
        ));
    }
    if has_syntax_errors {
        // Nothing but `Lint/Syntax` is produced for a broken parse, and that
        // rule is never suppressible, so building real directives here would
        // be wasted work: an empty set behaves identically.
        return FileResult {
            diagnostics: finish(syntax_diagnostics, source, &Directives::default(), settings),
            node_count: 0,
            has_syntax_errors,
        };
    }

    let directives = Directives::from_parsed(parsed);
    let mut ctx = Context::new(source, parsed, directives);

    rules.file_start(&mut ctx);
    let node_count = {
        let mut walker = Walker { rules, ctx: &mut ctx, node_count: 0 };
        walk(&parsed.root(), &mut walker);
        walker.node_count
    };
    rules.file_end(&mut ctx);

    let (diagnostics, directives) = ctx.into_parts();
    FileResult {
        diagnostics: finish(diagnostics, source, &directives, settings),
        node_count,
        has_syntax_errors,
    }
}

/// Drops disabled diagnostics, applies severity overrides, orders the
/// survivors by position, and dedups repeats.
///
/// A diagnostic is dropped when its rule is disabled in `settings`, or when
/// `directives` disables it (by name or via `# rubocop:disable all`) at its
/// line — unless it is [`SYNTAX_RULE`], which is never dropped or
/// re-severitied.
///
/// RuboCop's `add_offense` ignores a second offense from the same cop at the
/// same range, so a cop that fires twice on one node (and Prism, which often
/// emits a follow-up "assuming it is closing…" error at the same location)
/// reports once. The sort is stable, so the first-emitted diagnostic wins.
fn finish(
    mut diagnostics: Vec<Diagnostic>,
    source: &SourceFile,
    directives: &Directives,
    settings: &FileSettings,
) -> Vec<Diagnostic> {
    diagnostics.retain(|d| {
        if d.rule == SYNTAX_RULE {
            return true;
        }
        if !settings.is_enabled(d.rule) {
            return false;
        }
        let line = source.line_col(d.span.start).line;
        !directives.is_disabled(d.rule, line) && !directives.all_disabled_at(line)
    });
    diagnostics.sort_by_key(|d| (d.span.start, d.span.end, d.rule));
    diagnostics.dedup_by(|b, a| a.rule == b.rule && a.span == b.span);
    for diagnostic in &mut diagnostics {
        if diagnostic.rule != SYNTAX_RULE {
            if let Some(severity) = settings.severity_override(diagnostic.rule) {
                diagnostic.severity = severity;
            }
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use ruby_ast::NodeKind;
    use ruby_source::Span;

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

    /// Dispatch that pushes two fixed diagnostics — `Fake/A` at line 1 and
    /// `Fake/B` at line 3 — from `file_end`, regardless of tree contents.
    /// Exercises [`lint_parsed_with`]'s settings/directives filtering
    /// without needing real rules.
    struct FakeRules;

    impl Dispatch for FakeRules {
        fn file_start(&mut self, _ctx: &mut Context<'_>) {}
        fn enter(&mut self, _kind: NodeKind, _node: &Node<'_>, _ctx: &mut Context<'_>) {}
        fn leave(&mut self, _kind: NodeKind, _node: &Node<'_>, _ctx: &mut Context<'_>) {}
        fn file_end(&mut self, ctx: &mut Context<'_>) {
            let line1 = line_start(ctx.source().bytes(), 1);
            let line3 = line_start(ctx.source().bytes(), 3);
            ctx.push(Diagnostic::new(
                "Fake/A",
                Span::new(line1, line1 + 1),
                Severity::Warning,
                "fake a",
            ));
            ctx.push(Diagnostic::new(
                "Fake/B",
                Span::new(line3, line3 + 1),
                Severity::Warning,
                "fake b",
            ));
        }
    }

    /// Byte offset where 1-based `line` starts in `bytes`.
    fn line_start(bytes: &[u8], line: u32) -> u32 {
        if line == 1 {
            return 0;
        }
        let mut seen = 1u32;
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                seen += 1;
                if seen == line {
                    return u32::try_from(i + 1).unwrap();
                }
            }
        }
        panic!("source has no line {line}");
    }

    fn fake_rules(src: &[u8]) -> FileResult {
        let source = SourceFile::new("a.rb", src.to_vec());
        let parsed = Parsed::parse(&source);
        lint_parsed_with(&parsed, &mut FakeRules, &FileSettings::all_enabled())
    }

    fn fake_rules_with(src: &[u8], settings: &FileSettings) -> FileResult {
        let source = SourceFile::new("a.rb", src.to_vec());
        let parsed = Parsed::parse(&source);
        lint_parsed_with(&parsed, &mut FakeRules, settings)
    }

    const PLAIN_SOURCE: &[u8] = b"x = 1\ny = 2\nz = 3\n";

    #[test]
    fn all_rules_enabled_by_default() {
        let result = fake_rules(PLAIN_SOURCE);
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/A", "Fake/B"]);
    }

    #[test]
    fn disabled_rule_is_dropped() {
        let mut settings = FileSettings::all_enabled();
        settings.disable("Fake/A");
        let result = fake_rules_with(PLAIN_SOURCE, &settings);
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/B"]);
    }

    #[test]
    fn own_line_directive_disables_for_rest_of_file() {
        let source = b"x = 1\n# rubocop:disable Fake/B\nz = 3\n";
        let result = fake_rules(source);
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/A"]);
    }

    #[test]
    fn inline_directive_disables_only_its_own_line() {
        let source = b"x = 1 # rubocop:disable Fake/A\ny = 2\nz = 3\n";
        let result = fake_rules(source);
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/B"]);
    }

    #[test]
    fn severity_override_applies_to_survivors() {
        let mut settings = FileSettings::all_enabled();
        settings.set_severity("Fake/B", Severity::Error);
        let result = fake_rules_with(PLAIN_SOURCE, &settings);
        let b = result.diagnostics.iter().find(|d| d.rule == "Fake/B").unwrap();
        assert_eq!(b.severity, Severity::Error);
        let a = result.diagnostics.iter().find(|d| d.rule == "Fake/A").unwrap();
        assert_eq!(a.severity, Severity::Warning);
    }

    #[test]
    fn disable_all_never_suppresses_syntax_errors() {
        let source = SourceFile::new("a.rb", b"# rubocop:disable all\ndef foo(\n".to_vec());
        let result = lint_file(&source, &mut NoRules);
        assert!(result.has_syntax_errors);
        assert!(!result.diagnostics.is_empty());
        assert!(result.diagnostics.iter().all(|d| d.rule == SYNTAX_RULE));
    }

    #[test]
    fn disabling_syntax_rule_is_a_no_op() {
        let mut settings = FileSettings::all_enabled();
        settings.disable(SYNTAX_RULE);
        let source = SourceFile::new("a.rb", b"def foo(\n".to_vec());
        let parsed = Parsed::parse(&source);
        let result = lint_parsed_with(&parsed, &mut NoRules, &settings);
        assert!(result.has_syntax_errors);
        assert!(!result.diagnostics.is_empty());
        assert!(result.diagnostics.iter().all(|d| d.rule == SYNTAX_RULE));
    }

    /// Dispatch that records the full ancestor-kind path (outermost first)
    /// and immediate parent kind every time it enters a `CallNode`.
    #[derive(Default)]
    struct AncestorRecorder {
        paths: Vec<Vec<NodeKind>>,
        parents: Vec<Option<NodeKind>>,
    }

    impl Dispatch for AncestorRecorder {
        fn file_start(&mut self, _ctx: &mut Context<'_>) {}
        fn enter(&mut self, kind: NodeKind, _node: &Node<'_>, ctx: &mut Context<'_>) {
            if kind == NodeKind::CallNode {
                self.paths.push(ctx.ancestors().iter().map(|a| a.kind).collect());
                self.parents.push(ctx.parent().map(|p| p.kind));
            }
        }
        fn leave(&mut self, kind: NodeKind, _node: &Node<'_>, ctx: &mut Context<'_>) {
            if kind == NodeKind::CallNode {
                // The node itself must never be visible on its own stack.
                assert!(!ctx.ancestors().iter().any(|a| a.kind == NodeKind::CallNode));
            }
        }
        fn file_end(&mut self, _ctx: &mut Context<'_>) {}
    }

    #[test]
    fn ancestors_reflect_true_nesting_at_call_node() {
        let source =
            SourceFile::new("a.rb", b"def foo(a)\n  if a\n    bar(a)\n  end\nend\n".to_vec());
        let mut recorder = AncestorRecorder::default();
        lint_file(&source, &mut recorder);

        assert_eq!(recorder.paths.len(), 1);
        assert_eq!(
            recorder.paths[0],
            vec![
                NodeKind::ProgramNode,
                NodeKind::StatementsNode,
                NodeKind::DefNode,
                NodeKind::StatementsNode,
                NodeKind::IfNode,
                NodeKind::StatementsNode,
            ]
        );
        assert_eq!(recorder.parents[0], Some(NodeKind::StatementsNode));
        assert_eq!(recorder.parents[0], recorder.paths[0].last().copied());
    }
}

use std::borrow::Cow;

use ruby_ast::{walk, Node, NodeExt, Parsed, Visitor};
use ruby_directives::Directives;
use ruby_source::SourceFile;

use crate::context::{Context, NodeInfo};
use crate::diagnostic::{Diagnostic, Severity};
use crate::rule::Dispatch;
use crate::settings::FileSettings;

/// Cop name RuboCop uses for parse errors.
pub const SYNTAX_RULE: &str = "Lint/Syntax";

/// `Lint/RedundantCopDisableDirective`'s cop name, RuboCop's
/// `DirectiveComment::LINT_REDUNDANT_DIRECTIVE_COP`: this cop is wired directly into RuboCop's
/// registry/team-runner as un-suppressible by *any* directive, including `# rubocop:disable
/// all` on its own comment -- otherwise the very comment this cop is warning about would always
/// silence the warning. See [`finish`]'s directive-suppression retain.
pub const REDUNDANT_DISABLE_DIRECTIVE_RULE: &str = "Lint/RedundantCopDisableDirective";

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
    lint_parsed_with_injected(parsed, rules, settings, &[])
}

/// Like [`lint_parsed_with`], but each `(rule, line)` pair in `injected` additionally becomes a
/// synthetic [`Diagnostic`] (spanning that whole 1-based source line, [`Severity::Convention`])
/// visible to [`crate::rule::Rule::file_finish`] through its `reported` argument, without
/// appearing anywhere in the returned [`FileResult::diagnostics`]. `Lint/RedundantCopDisableDirective`
/// is the only rule that needs this: its upstream RuboCop spec constructs the cop with a
/// literal `offenses` array simulating diagnostics from other cops that never actually ran in
/// the same investigation. The fixture harness (`crates/rules/tests/fixtures.rs`) replays those
/// examples by injecting synthetic diagnostics here; [`crate::fix_file`]'s per-round re-parse
/// means a plain `(rule, line)` pair -- re-resolved to a byte span against *this* call's own
/// `parsed` every time -- stays correct across fix iterations, whereas a pre-built [`Diagnostic`]
/// with a stale byte span from an earlier round's source would not.
pub fn lint_parsed_with_injected<D: Dispatch>(
    parsed: &Parsed<'_>,
    rules: &mut D,
    settings: &FileSettings,
    injected: &[(&'static str, u32)],
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

    let mut reported = ctx.take_diagnostics();
    reported.sort_by_key(|d| (d.span.start, d.span.end));
    if injected.is_empty() {
        rules.file_finish(&mut ctx, &reported);
    } else {
        let total_lines = u32::try_from(source.lines().line_starts().len()).unwrap_or(1).max(1);
        let mut visible = reported.clone();
        visible.extend(injected.iter().map(|&(rule, line)| {
            // Upstream's injected `RuboCop::Cop::Offense`s carry arbitrary, sometimes
            // out-of-file line numbers (e.g. a `FakeLocation.new(line: 7)` in a two-line
            // fixture): they only ever matter for `Range#cover?`-style membership against an
            // open-ended (`Float::INFINITY`) disabled range, never for locating real source
            // text, so clamping to the file's last real line preserves every comparison that
            // matters without indexing past the line table.
            let line = line.min(total_lines);
            let start = source.lines().line_start(line);
            let end = start + u32::try_from(source.line_text(line).len()).unwrap_or(0);
            Diagnostic::new(
                rule,
                ruby_source::Span::new(start, end),
                Severity::Convention,
                "injected",
            )
        }));
        visible.sort_by_key(|d| (d.span.start, d.span.end));
        rules.file_finish(&mut ctx, &visible);
    }
    reported.append(&mut ctx.take_diagnostics());

    let (_, directives) = ctx.into_parts();
    FileResult {
        diagnostics: finish(reported, source, &directives, settings),
        node_count,
        has_syntax_errors,
    }
}

/// Drops disabled diagnostics, applies severity overrides, orders the
/// survivors by position, and dedups repeats.
///
/// A diagnostic is dropped when its rule is disabled in `settings`, or when
/// `directives` disables it (by name or via `# rubocop:disable all`) at its
/// line -- unless it is [`SYNTAX_RULE`], which is never dropped or
/// re-severitied.
///
/// A rule `settings` disables is still reported -- from strictly after the
/// enabling comment's line onward -- when `directives` shows it was "opted
/// in" by a `# rubocop:enable <Rule>` directive naming it exactly. This
/// mirrors RuboCop's `Cop::Team#roundup_relevant_cops`
/// (`lib/rubocop/cop/team.rb:178-186`, RuboCop 1.82.1): `next true if
/// processed_source.comment_config.cop_opted_in?(cop)` runs *before* the
/// `@registry.enabled?(cop, @config)` check, so a cop disabled by
/// `AllCops: DisabledByDefault: true`, an explicit `Enabled: false`, or any
/// other configuration reason reactivates for the whole file once named in
/// an `enable` directive -- `CommentConfig#cop_opted_in?`
/// (`lib/rubocop/comment_config.rb:48-50`) never consults the
/// configuration at all. [`crate::rule::Rule`]s themselves are not gated by
/// `settings` here (the caller decides which rules run at all; see
/// `RuleSet::only` in `crates/cli`, which must include an opted-in rule for
/// its diagnostics to reach this filter in the first place).
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
        let line = source.line_col(d.span.start).line;
        if !settings.is_enabled(d.rule) {
            return directives.is_opted_in(d.rule)
                && !directives.is_disabled_for_opted_in_cop(d.rule, line);
        }
        // `Lint/RedundantCopDisableDirective` is excluded from `all`/`Lint` department
        // expansion (`DirectiveComment#exclude_lint_department_cops`), so `# rubocop:disable
        // all` never silences it -- but naming it explicitly still does, like any other cop.
        if d.rule == REDUNDANT_DISABLE_DIRECTIVE_RULE {
            return !directives.is_disabled_by_name(d.rule, line);
        }
        !directives.is_disabled(d.rule, line) && !directives.all_disabled_at(line)
    });
    diagnostics.sort_by_key(|d| (d.span.start, d.span.end, d.rule));
    diagnostics.dedup_by(|b, a| a.rule == b.rule && a.span == b.span);
    for diagnostic in &mut diagnostics {
        if diagnostic.rule != SYNTAX_RULE {
            if let Some(severity) = settings.severity_override(diagnostic.rule) {
                diagnostic.severity = severity;
            }
            if let Some(annotated) = settings.annotate(diagnostic.rule, &diagnostic.message) {
                diagnostic.message = Cow::Owned(annotated);
            }
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ruby_ast::NodeKind;
    use ruby_source::Span;

    use super::*;
    use crate::rule::NoRules;
    use crate::settings::{Annotation, Annotations};

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

    /// Dispatch that pushes two fixed diagnostics — `Fake/Aa` at line 1 and
    /// `Fake/Bb` at line 3 — from `file_end`, regardless of tree contents.
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
                "Fake/Aa",
                Span::new(line1, line1 + 1),
                Severity::Warning,
                "fake a",
            ));
            ctx.push(Diagnostic::new(
                "Fake/Bb",
                Span::new(line3, line3 + 1),
                Severity::Warning,
                "fake b",
            ));
        }
        fn file_finish(&mut self, _ctx: &mut Context<'_>, _reported: &[Diagnostic]) {}
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
        assert_eq!(rules, vec!["Fake/Aa", "Fake/Bb"]);
    }

    #[test]
    fn disabled_rule_is_dropped() {
        let mut settings = FileSettings::all_enabled();
        settings.disable("Fake/Aa");
        let result = fake_rules_with(PLAIN_SOURCE, &settings);
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/Bb"]);
    }

    // Ports of RuboCop::Cop::Team#roundup_relevant_cops / CommentConfig#cop_opted_in?
    // (team.rb:178-186, comment_config.rb:48-50, RuboCop 1.82.1): a cop `settings`
    // disabled is reactivated for the file by a `# rubocop:enable <Cop>` directive
    // naming it exactly, but only from strictly after that directive's line onward.

    #[test]
    fn opted_in_rule_is_reported_after_its_enable_line() {
        // `Fake/Bb` fires at line 3; naming it on line 2 (before the offense)
        // reactivates it there.
        let mut settings = FileSettings::all_enabled();
        settings.disable("Fake/Bb");
        let source = b"x = 1\n# rubocop:enable Fake/Bb\nz = 3\n";
        let result = fake_rules_with(source, &settings);
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/Aa", "Fake/Bb"]);
    }

    #[test]
    fn opted_in_rule_stays_suppressed_up_to_and_including_its_enable_line() {
        // Naming `Fake/Bb` on line 4 -- after its line-3 offense -- leaves
        // that offense suppressed: the synthetic config-disabled range only
        // closes strictly after the enable directive's own line.
        let mut settings = FileSettings::all_enabled();
        settings.disable("Fake/Bb");
        let source = b"x = 1\ny = 2\nz = 3\n# rubocop:enable Fake/Bb\n";
        let result = fake_rules_with(source, &settings);
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/Aa"]);
    }

    #[test]
    fn enabling_a_different_rule_does_not_opt_this_one_in() {
        let mut settings = FileSettings::all_enabled();
        settings.disable("Fake/Bb");
        let source = b"x = 1\n# rubocop:enable Fake/Aa\nz = 3\n";
        let result = fake_rules_with(source, &settings);
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/Aa"]);
    }

    #[test]
    fn disabled_rule_without_any_directive_stays_dropped() {
        let mut settings = FileSettings::all_enabled();
        settings.disable("Fake/Bb");
        let result = fake_rules_with(PLAIN_SOURCE, &settings);
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/Aa"]);
    }

    #[test]
    fn own_line_directive_disables_for_rest_of_file() {
        let source = b"x = 1\n# rubocop:disable Fake/Bb\nz = 3\n";
        let result = fake_rules(source);
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/Aa"]);
    }

    #[test]
    fn inline_directive_disables_only_its_own_line() {
        let source = b"x = 1 # rubocop:disable Fake/Aa\ny = 2\nz = 3\n";
        let result = fake_rules(source);
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/Bb"]);
    }

    #[test]
    fn severity_override_applies_to_survivors() {
        let mut settings = FileSettings::all_enabled();
        settings.set_severity("Fake/Bb", Severity::Error);
        let result = fake_rules_with(PLAIN_SOURCE, &settings);
        let b = result.diagnostics.iter().find(|d| d.rule == "Fake/Bb").unwrap();
        assert_eq!(b.severity, Severity::Error);
        let a = result.diagnostics.iter().find(|d| d.rule == "Fake/Aa").unwrap();
        assert_eq!(a.severity, Severity::Warning);
    }

    /// Dispatch that reports one fixed `Style/StringLiterals` diagnostic
    /// with RuboCop's real default message, regardless of tree contents.
    struct StringLiteralsRule;

    impl Dispatch for StringLiteralsRule {
        fn file_start(&mut self, _ctx: &mut Context<'_>) {}
        fn enter(&mut self, _kind: NodeKind, _node: &Node<'_>, _ctx: &mut Context<'_>) {}
        fn leave(&mut self, _kind: NodeKind, _node: &Node<'_>, _ctx: &mut Context<'_>) {}
        fn file_end(&mut self, ctx: &mut Context<'_>) {
            ctx.push(Diagnostic::new(
                "Style/StringLiterals",
                Span::new(0, 1),
                Severity::Convention,
                "Prefer single-quoted strings when you don't need string interpolation or \
                 special symbols.",
            ));
        }
        fn file_finish(&mut self, _ctx: &mut Context<'_>, _reported: &[Diagnostic]) {}
    }

    #[test]
    fn display_style_guide_appends_the_cops_style_guide_url() {
        let mut annotations = Annotations::new(true, false);
        annotations.insert(
            "Style/StringLiterals",
            Annotation {
                style_guide_url: Some(
                    "https://rubystyle.guide#consistent-string-literals".to_string(),
                ),
                ..Annotation::default()
            },
        );
        let mut settings = FileSettings::all_enabled();
        settings.set_annotations(Arc::new(annotations));

        let source = SourceFile::new("a.rb", b"x = \"a\"\n".to_vec());
        let parsed = Parsed::parse(&source);
        let result = lint_parsed_with(&parsed, &mut StringLiteralsRule, &settings);

        let offense = &result.diagnostics[0];
        assert!(
            offense.message.ends_with(" (https://rubystyle.guide#consistent-string-literals)"),
            "unexpected message: {}",
            offense.message
        );
    }

    #[test]
    fn default_settings_leave_messages_unannotated() {
        let source = SourceFile::new("a.rb", b"x = \"a\"\n".to_vec());
        let parsed = Parsed::parse(&source);
        let result =
            lint_parsed_with(&parsed, &mut StringLiteralsRule, &FileSettings::all_enabled());

        assert_eq!(
            result.diagnostics[0].message,
            "Prefer single-quoted strings when you don't need string interpolation or special \
             symbols."
        );
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
        fn file_finish(&mut self, _ctx: &mut Context<'_>, _reported: &[Diagnostic]) {}
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

    /// Dispatch that pushes `Fake/Aa` at line 1 on entering the program node
    /// and, in `file_finish`, reports `Fake/Bb` at the same line after
    /// checking it saw `Fake/Aa` in `reported` — the diagnostics accumulated
    /// by every rule so far, before directive suppression and dedup.
    struct FinishRules;

    impl Dispatch for FinishRules {
        fn file_start(&mut self, _ctx: &mut Context<'_>) {}
        fn enter(&mut self, kind: NodeKind, _node: &Node<'_>, ctx: &mut Context<'_>) {
            if kind == NodeKind::ProgramNode {
                ctx.push(Diagnostic::new("Fake/Aa", Span::new(0, 1), Severity::Warning, "fake a"));
            }
        }
        fn leave(&mut self, _kind: NodeKind, _node: &Node<'_>, _ctx: &mut Context<'_>) {}
        fn file_end(&mut self, _ctx: &mut Context<'_>) {}
        fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
            assert_eq!(reported.iter().map(|d| d.rule).collect::<Vec<_>>(), vec!["Fake/Aa"]);
            ctx.push(Diagnostic::new("Fake/Bb", Span::new(0, 1), Severity::Warning, "fake b"));
        }
    }

    #[test]
    fn file_finish_sees_prior_diagnostics_and_can_report_more() {
        let source = SourceFile::new("a.rb", b"x = 1\n".to_vec());
        let parsed = Parsed::parse(&source);
        let result = lint_parsed_with(&parsed, &mut FinishRules, &FileSettings::all_enabled());
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/Aa", "Fake/Bb"]);
    }

    #[test]
    fn file_finish_sees_a_diagnostic_a_directive_will_suppress() {
        // `Fake/Aa` fires at line 1, same line as the directive disabling
        // it, so it never reaches the final result — but `FinishRules`
        // still asserts it was visible in `reported` before that
        // suppression happened.
        let source = SourceFile::new("a.rb", b"# rubocop:disable Fake/Aa\nx = 1\n".to_vec());
        let parsed = Parsed::parse(&source);
        let result = lint_parsed_with(&parsed, &mut FinishRules, &FileSettings::all_enabled());
        let rules: Vec<_> = result.diagnostics.iter().map(|d| d.rule).collect();
        assert_eq!(rules, vec!["Fake/Bb"]);
    }
}

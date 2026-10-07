//! `Sorbet/EmptyLineAfterSig`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/empty_line_after_sig.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Extra empty line or comment detected";

/// Ensures that there are no blank lines after signatures
#[derive(Debug, Clone)]
pub struct EmptyLineAfterSig;

impl Rule for EmptyLineAfterSig {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/EmptyLineAfterSig",
        department: Department::Sorbet,
        summary: "Ensures that there are no blank lines after signatures",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(statements) = node.as_statements_node() else { return };
        let body: Vec<Node<'_>> = statements.body().iter().collect();
        for pair in body.windows(2) {
            let (sig, next) = (&pair[0], &pair[1]);
            if is_signature(sig) && is_sig_or_signable_method_definition(next) {
                check(sig, next, ctx);
            }
        }
    }
}

/// `${ any_def (send nil? {:attr :attr_reader :attr_writer :attr_accessor} ...) #signature? }`
fn is_sig_or_signable_method_definition(node: &Node<'_>) -> bool {
    if node.as_def_node().is_some() {
        return true;
    }
    if let Some(call) = node.as_call_node() {
        if !call.is_safe_navigation()
            && call.block().is_none_or(|b| b.as_block_node().is_none())
            && call.receiver().is_none()
            && matches!(
                call.name().as_slice(),
                b"attr" | b"attr_reader" | b"attr_writer" | b"attr_accessor"
            )
        {
            return true;
        }
    }
    is_signature(node)
}

fn is_rubocop_line(line: &[u8]) -> bool {
    line.trim_ascii().starts_with(b"# rubocop:")
}

fn check(sig: &Node<'_>, definition: &Node<'_>, ctx: &mut Context<'_>) {
    let end_of_sig = sig.span().end;
    let start_of_definition = definition.span().start;
    let source = ctx.source().bytes();
    let between = &source[end_of_sig as usize..start_of_definition as usize];
    // Fallbacks handle same line edge case.
    let begin_offset = between.iter().position(|&b| b == b'\n').unwrap_or(0);
    if between.is_empty() {
        return;
    }
    let end_offset = between.iter().rposition(|&b| b == b'\n').unwrap_or(between.len() - 1);
    let (Ok(begin), Ok(end)) = (
        u32::try_from(end_of_sig as usize + begin_offset + 1),
        u32::try_from(end_of_sig as usize + end_offset + 1),
    ) else {
        return;
    };
    let range = Span::new(begin, end);
    if range.start == range.end {
        return;
    }
    // `range.single_line?`
    if ctx.line_col(range.start).line == ctx.line_col(range.end).line {
        return;
    }
    let text = ctx.text(range).to_vec();
    let lines: Vec<&[u8]> = text.split_inclusive(|&b| b == b'\n').collect();
    // `contains_only_rubocop_directives?`
    if lines.iter().all(|line| is_rubocop_line(line)) {
        return;
    }

    let (rubocop_lines, other_lines): (Vec<&[u8]>, Vec<&[u8]>) =
        lines.iter().partition(|line| is_rubocop_line(line));
    let mut edits = Vec::new();
    if !other_lines.is_empty() {
        let joined: Vec<u8> = other_lines.concat();
        let trimmed_start = joined.iter().position(|&b| b != b'\n').unwrap_or(joined.len());
        let mut inserted: Vec<u8> = Vec::new();
        let mut newlines = 0;
        for &b in &joined[trimmed_start..] {
            if b == b'\n' {
                newlines += 1;
                if newlines > 1 {
                    continue;
                }
            } else {
                newlines = 0;
            }
            inserted.push(b);
        }
        let sig_line = ctx.line_col(sig.span().start).line;
        edits.push(Edit::insert(ctx.line_span(sig_line).start, inserted));
    }
    edits.push(Edit::replace(range, rubocop_lines.concat()));
    ctx.report_with_fix(
        &EmptyLineAfterSig::META,
        range,
        MSG,
        Fix { applicability: Applicability::Safe, edits },
    );
}

/// `(const {nil? cbase} :T)`.
fn is_t_const(node: &Node<'_>) -> bool {
    if let Some(c) = node.as_constant_read_node() {
        return c.name().as_slice() == b"T";
    }
    node.as_constant_path_node()
        .is_some_and(|p| p.parent().is_none() && p.name().is_some_and(|n| n.as_slice() == b"T"))
}

/// `(const (const {nil? cbase} :T) :Sig)`.
fn is_t_sig_const(node: &Node<'_>) -> bool {
    node.as_constant_path_node().is_some_and(|p| {
        p.name().is_some_and(|n| n.as_slice() == b"Sig")
            && p.parent().is_some_and(|parent| is_t_const(&parent))
    })
}

/// `(const (const (const {nil? cbase} :T) :Sig) :WithoutRuntime)`.
fn is_t_sig_without_runtime_const(node: &Node<'_>) -> bool {
    node.as_constant_path_node().is_some_and(|p| {
        p.name().is_some_and(|n| n.as_slice() == b"WithoutRuntime")
            && p.parent().is_some_and(|parent| is_t_sig_const(&parent))
    })
}

/// `SignatureHelp`'s `signature?`: `bare_sig?`, `sig_with_runtime?` or
/// `sig_without_runtime?` -- a `block` (not numblock/itblock) with empty
/// `(args)` around `sig` / `sig(:final)`.
fn is_signature(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.is_safe_navigation() || call.name().as_slice() != b"sig" {
        return false;
    }
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return false };
    if let Some(params) = block.parameters() {
        let Some(params) = params.as_block_parameters_node() else { return false };
        if params.parameters().is_some() || params.locals().iter().next().is_some() {
            return false;
        }
    }
    if let Some(arguments) = call.arguments() {
        let mut it = arguments.arguments().iter();
        let ok = matches!((it.next(), it.next()), (Some(a), None)
            if a.as_symbol_node().is_some_and(|s| s.unescaped() == b"final"));
        if !ok {
            return false;
        }
    }
    match call.receiver() {
        None => true,
        Some(receiver) => is_t_sig_const(&receiver) || is_t_sig_without_runtime_const(&receiver),
    }
}

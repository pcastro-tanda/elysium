//! `Sorbet/CapitalizedTypeParameters`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/signatures/capitalized_type_parameters.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Type parameters must be capitalized.";

/// Ensure type parameters used in generic methods are always capitalized.
#[derive(Debug, Clone)]
pub struct CapitalizedTypeParameters;

impl Rule for CapitalizedTypeParameters {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/CapitalizedTypeParameters",
        department: Department::Sorbet,
        summary: "Ensures that type parameters are capitalized.",
        explanation: "Ensure type parameters used in generic methods are always capitalized.\n\n```ruby\n# bad\nsig { type_parameters(:x).params(a: T.type_parameter(:x)).void }\ndef foo(a); end\n\n# good\nsig { type_parameters(:X).params(a: T.type_parameter(:X)).void }\ndef foo(a: 1); end\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Symbol#inspect of the corrected value is approximated: names made of \
                      alphanumerics, `_` and non-ASCII characters (with an optional trailing \
                      `?`, `!` or `=`) print bare, anything else is double-quoted.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if is_signature(&call) {
            on_signature(&call, ctx);
        }
        if is_t_type_parameter(&call) {
            check_type_parameters_case(&call, ctx);
        }
    }
}

/// `{#bare_sig? #sig_with_runtime? #sig_without_runtime?}`.
fn is_signature(call: &CallNode<'_>) -> bool {
    if call.is_safe_navigation() || call.name().as_slice() != b"sig" {
        return false;
    }
    let Some(block_node) = call.block() else { return false };
    let Some(block) = block_node.as_block_node() else { return false };
    // `(args)`: no parameters (numblock/itblock are other node types).
    match block.parameters() {
        None => {}
        Some(parameters) => {
            let Some(params) = parameters.as_block_parameters_node() else { return false };
            if params.parameters().is_some() || params.locals().iter().next().is_some() {
                return false;
            }
        }
    }
    // `(sym :final)?`
    if let Some(arguments) = call.arguments() {
        let mut args = arguments.arguments().iter();
        let (Some(arg), None) = (args.next(), args.next()) else { return false };
        let Some(sym) = arg.as_symbol_node() else { return false };
        if sym.unescaped() != b"final" {
            return false;
        }
    }
    match call.receiver() {
        None => true,
        Some(receiver) => {
            // (const (const {nil? cbase} :T) :Sig) / (... :WithoutRuntime)
            let Some(path) = receiver.as_constant_path_node() else { return false };
            let Some(name) = path.name() else { return false };
            let Some(parent) = path.parent() else { return false };
            match name.as_slice() {
                b"Sig" => is_t_const(&parent),
                b"WithoutRuntime" => {
                    let Some(sig) = parent.as_constant_path_node() else { return false };
                    sig.name().is_some_and(|n| n.as_slice() == b"Sig")
                        && sig.parent().is_some_and(|t| is_t_const(&t))
                }
                _ => false,
            }
        }
    }
}

/// `(const {nil? cbase} :T)`.
fn is_t_const(node: &Node<'_>) -> bool {
    match node {
        Node::ConstantReadNode { .. } => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"T")
        }
        Node::ConstantPathNode { .. } => node.as_constant_path_node().is_some_and(|path| {
            path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"T")
        }),
        _ => false,
    }
}

/// `(send (const {nil? | cbase} :T) :type_parameter ...)` (send or csend).
fn is_t_type_parameter(call: &CallNode<'_>) -> bool {
    call.name().as_slice() == b"type_parameter"
        && call.block().is_none_or(|block| block.as_block_node().is_none())
        && call.receiver().is_some_and(|receiver| is_t_const(&receiver))
}

fn on_signature(sig: &CallNode<'_>, ctx: &mut Context<'_>) {
    let Some(block_node) = sig.block() else { return };
    let Some(block) = block_node.as_block_node() else { return };
    let Some(body) = block.body() else { return };
    let Some(statements) = body.as_statements_node() else { return };
    let mut stmts = statements.body().iter();
    let (Some(first), None) = (stmts.next(), stmts.next()) else { return };
    let mut current = first;
    // `while send&.send_type?`
    loop {
        let Some(call) = current.as_call_node() else { return };
        if call.is_safe_navigation()
            || call.block().is_some_and(|block| block.as_block_node().is_some())
        {
            return;
        }
        // (send nil? :type_parameters ...)
        if call.receiver().is_none() && call.name().as_slice() == b"type_parameters" {
            check_type_parameters_case(&call, ctx);
        }
        let Some(receiver) = call.receiver() else { return };
        current = receiver;
    }
}

fn check_type_parameters_case(call: &CallNode<'_>, ctx: &mut Context<'_>) {
    let Some(arguments) = call.arguments() else { return };
    for arg in &arguments.arguments() {
        let Some(symbol) = arg.as_symbol_node() else { continue };
        let value = String::from_utf8_lossy(symbol.unescaped()).into_owned();
        if starts_a_line_with_capital(&value) {
            continue;
        }
        let span = arg.span();
        let replacement = symbol_inspect(&capitalize(&value));
        ctx.report_with_fix(
            &CapitalizedTypeParameters::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            },
        );
    }
}

/// `value =~ /^[A-Z]/`: `^` also matches after any newline.
fn starts_a_line_with_capital(value: &str) -> bool {
    value.split('\n').any(|line| line.starts_with(|c: char| c.is_ascii_uppercase()))
}

/// `String#capitalize`.
fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let mut out = String::with_capacity(value.len());
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
    }
    out.extend(chars.flat_map(char::to_lowercase));
    out
}

/// `Symbol#inspect` (approximation: see `blind_spots`).
fn symbol_inspect(value: &str) -> String {
    let body = value.strip_suffix(['?', '!', '=']).unwrap_or(value);
    let simple = body.chars().next().is_some_and(|c| !c.is_ascii_digit())
        && body.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || !c.is_ascii());
    if simple {
        format!(":{value}")
    } else {
        let mut out = String::from(":\"");
        for c in value.chars() {
            match c {
                '"' | '\\' => {
                    out.push('\\');
                    out.push(c);
                }
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                _ => out.push(c),
            }
        }
        out.push('"');
        out
    }
}

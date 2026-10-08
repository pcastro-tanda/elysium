//! `Sorbet/KeywordArgumentOrdering`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/signatures/keyword_argument_ordering.rs`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Optional keyword arguments must be at the end of the parameter list.";

/// Checks for the ordering of keyword arguments required by sorbet-runtime.
#[derive(Debug, Clone)]
pub struct KeywordArgumentOrdering;

impl Rule for KeywordArgumentOrdering {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/KeywordArgumentOrdering",
        department: Department::Sorbet,
        summary: "Enforces a compatible keyword arguments with Sorbet.",
        explanation: "Checks for the ordering of keyword arguments required by sorbet-runtime. All keyword arguments must be at the end of the parameters list, and all keyword arguments with a default value must be after those without default values.\n\n```ruby\n# bad\nsig { params(a: Integer, b: String).void }\ndef foo(a: 1, b:); end\n\n# good\nsig { params(b: String, a: Integer).void }\ndef foo(b:, a: 1); end\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
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
        for (index, statement) in body.iter().enumerate() {
            let Some(call) = statement.as_call_node() else { continue };
            if !is_signature(&call) {
                continue;
            }
            // `method_node.arguments`: only method definitions yield
            // parameters that matter here.
            let Some(def) = body.get(index + 1).and_then(Node::as_def_node) else { continue };
            let Some(parameters) = def.parameters() else { continue };

            // Source order, reversed.
            let mut params: Vec<Node<'_>> = Vec::new();
            params.extend(parameters.requireds().iter());
            params.extend(parameters.optionals().iter());
            params.extend(parameters.rest());
            params.extend(parameters.posts().iter());
            params.extend(parameters.keywords().iter());
            params.extend(parameters.keyword_rest());
            params.extend(parameters.block().map(|b| b.as_node()));

            let mut out_of_kwoptarg = false;
            for param in params.iter().rev() {
                let is_kwopt = param.as_optional_keyword_parameter_node().is_some();
                if !(is_kwopt
                    || param.as_block_parameter_node().is_some()
                    || param.as_keyword_rest_parameter_node().is_some())
                {
                    out_of_kwoptarg = true;
                }
                if is_kwopt && out_of_kwoptarg {
                    ctx.report(&Self::META, param.span(), MSG);
                }
            }
        }
    }
}

/// `Sorbet::SignatureHelp#signature?`: `sig`, `sig(:final)`, `T::Sig.sig`...
/// called with a literal block taking no parameters (whitequark `block`, not
/// `numblock`/`itblock`).
fn is_signature(call: &CallNode<'_>) -> bool {
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
        let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let [only] = args.as_slice() else { return false };
        let Some(sym) = only.as_symbol_node() else { return false };
        if sym.unescaped() != b"final" {
            return false;
        }
    }
    match call.receiver() {
        None => true,
        Some(receiver) => {
            matches!(const_name(&receiver).as_deref(), Some("T::Sig" | "T::Sig::WithoutRuntime"))
        }
    }
}

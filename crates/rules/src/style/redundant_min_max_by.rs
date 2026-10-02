//! `Style/RedundantMinMaxBy`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_min_max_by.rb`.
//!
//! RuboCop's `on_block`/`on_numblock`/`on_itblock` all fire on whitequark's
//! unified block-with-its-call node; Prism instead attaches a literal block
//! to the [`ruby_ast::node::CallNode`] it belongs to via `call.block()`, so
//! this rule dispatches on [`NodeKind::CallNode`] directly and inspects the
//! attached block for the `(args (arg $_x)) (lvar _x)` / numbered-`1` / `it`
//! shapes that upstream's three node-matchers require.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks whether `block`'s body is a single statement reading the local
/// variable `name` -- RuboCop's `(lvar _x)` / `(lvar :_1)` / `(lvar :it)`
/// body shape. An implicit `it`-block reference is Prism's
/// `ItLocalVariableReadNode`, not a `LocalVariableReadNode`.
fn body_reads_var(block_body: Option<Node<'_>>, name: &[u8]) -> bool {
    let Some(body) = block_body else { return false };
    let Some(stmts) = body.as_statements_node() else { return false };
    let children: Vec<Node<'_>> = stmts.body().iter().collect();
    let [only] = children.as_slice() else { return false };
    if name == b"it" && only.kind() == NodeKind::ItLocalVariableReadNode {
        return true;
    }
    let Some(lvar) = only.as_local_variable_read_node() else { return false };
    lvar.name().as_slice() == name
}

fn replacement(method: &[u8]) -> Option<&'static str> {
    match method {
        b"max_by" => Some("max"),
        b"min_by" => Some("min"),
        b"minmax_by" => Some("minmax"),
        _ => None,
    }
}

/// Identifies places where `max_by { ... }`, `min_by { ... }`, or
/// `minmax_by { ... }` can be replaced by `max`, `min`, or `minmax`.
///
/// # Examples
///
/// ```ruby
/// # bad
/// array.max_by { |x| x }
/// array.min_by { |x| x }
/// array.minmax_by { |x| x }
///
/// # good
/// array.max
/// array.min
/// array.minmax
/// ```
#[derive(Debug, Clone)]
pub struct RedundantMinMaxBy;

impl Rule for RedundantMinMaxBy {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantMinMaxBy",
        department: Department::Style,
        summary: "Identifies places where `max_by`/`min_by` can be replaced by `max`/`min`.",
        explanation: "\
Identifies places where `max_by { ... }`, `min_by { ... }`, or \
`minmax_by { ... }` can be replaced by `max`, `min`, or `minmax`.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let name = call.name();
        let name = name.as_slice();
        let Some(replacement) = replacement(name) else { return };

        let Some(block_node) = call.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };

        let message = match block.parameters() {
            Some(params) => match params.kind() {
                NodeKind::BlockParametersNode => {
                    let block_params = params.as_block_parameters_node().expect("kind matched");
                    let Some(inner) = block_params.parameters() else { return };
                    let required: Vec<Node<'_>> = inner.requireds().iter().collect();
                    let ([var], true) = (
                        required.as_slice(),
                        inner.optionals().is_empty()
                            && inner.rest().is_none()
                            && inner.posts().is_empty()
                            && inner.keywords().is_empty()
                            && inner.keyword_rest().is_none()
                            && inner.block().is_none(),
                    ) else {
                        return;
                    };
                    let Some(req) = var.as_required_parameter_node() else { return };
                    let var_name = req.name();
                    if !body_reads_var(block.body(), var_name.as_slice()) {
                        return;
                    }
                    let var_name = String::from_utf8_lossy(var_name.as_slice()).into_owned();
                    let method = String::from_utf8_lossy(name);
                    format!(
                        "Use `{replacement}` instead of `{method} {{ |{var_name}| {var_name} }}`."
                    )
                }
                NodeKind::NumberedParametersNode => {
                    let np = params.as_numbered_parameters_node().expect("kind matched");
                    if np.maximum() != 1 || !body_reads_var(block.body(), b"_1") {
                        return;
                    }
                    let method = String::from_utf8_lossy(name);
                    format!("Use `{replacement}` instead of `{method} {{ _1 }}`.")
                }
                NodeKind::ItParametersNode => {
                    if !body_reads_var(block.body(), b"it") {
                        return;
                    }
                    let method = String::from_utf8_lossy(name);
                    format!("Use `{replacement}` instead of `{method} {{ it }}`.")
                }
                _ => return,
            },
            None => return,
        };

        let Some(message_loc) = call.message_loc() else { return };
        let range = Span::new(message_loc.span().start, block_node.location().span().end);

        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, replacement.as_bytes().to_vec())],
            },
        );
    }
}

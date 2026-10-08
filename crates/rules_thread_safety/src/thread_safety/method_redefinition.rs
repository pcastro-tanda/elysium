//! `ThreadSafety/MethodRedefinition`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/method_redefinition.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Do not use `remove_method` followed by method definition.";

/// Avoid the thread-unsafe combination of `remove_method` followed by defining
/// a method with the same name; alias the method to itself instead.
#[derive(Debug, Clone)]
pub struct MethodRedefinition;

/// The unescaped value of a plain `str`/`sym` literal.
fn literal_value(node: &Node<'_>) -> Option<Vec<u8>> {
    if let Some(s) = node.as_string_node() {
        return Some(s.unescaped().to_vec());
    }
    node.as_symbol_node().map(|s| s.unescaped().to_vec())
}

impl Rule for MethodRedefinition {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/MethodRedefinition",
        department: Department::ThreadSafety,
        summary: "Do not use `remove_method` followed by method definition.",
        explanation: "Avoid the thread-unsafe combination of remove_method followed by defining a method with the same name. This can lead to a race condition, as these two actions are not atomic. As a safer alternative, consider aliasing the method to itself instead.",
        enabled_by_default: false,
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
        for pair in body.windows(2) {
            let Some(call) = pair[0].as_call_node() else { continue };
            if call.name().as_slice() != b"remove_method" || call.block().is_some() {
                continue;
            }
            let Some(def) = pair[1].as_def_node() else { continue };
            if def.receiver().is_some() {
                continue;
            }
            let Some(args) = call.arguments() else { continue };
            let args = args.arguments();
            let mut it = args.iter();
            let (Some(arg), None) = (it.next(), it.next()) else { continue };
            let Some(value) = literal_value(&arg) else { continue };
            if def.name().as_slice() != value.as_slice() {
                continue;
            }
            ctx.report(&Self::META, pair[0].span(), MSG);
        }
    }
}

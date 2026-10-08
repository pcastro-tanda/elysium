//! `Performance/BlockGivenWithExplicitBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/block_given_with_explicit_block.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{each_descendant, Node, NodeKind};

const MSG: &str = "Check block argument explicitly instead of using `block_given?`.";

/// What `enter` of a `def` learned about its block argument: `Some(name)` when
/// the def has a named `&block` parameter that is never reassigned.
type DefBlockArg = Option<Vec<u8>>;

/// Whether an assignment-target node (`LocalVariableTargetNode`, possibly nested
/// in multiple-assignment targets or splats) assigns the local `name`.
fn target_assigns(node: &Node<'_>, name: &[u8]) -> bool {
    if let Some(t) = node.as_local_variable_target_node() {
        return t.name().as_slice() == name;
    }
    if let Some(m) = node.as_multi_target_node() {
        return m.lefts().iter().any(|n| target_assigns(&n, name))
            || m.rest().is_some_and(|n| target_assigns(&n, name))
            || m.rights().iter().any(|n| target_assigns(&n, name));
    }
    node.as_splat_node().and_then(|s| s.expression()).is_some_and(|n| target_assigns(&n, name))
}

/// `` `(lvasgn %1 ...) ``: whether any descendant of `def_node` is a
/// whitequark `lvasgn` of the local `name` (plain and operator assignments,
/// multiple-assignment targets, `rescue => name`, `for name in`). Pattern
/// captures and named regexp captures are not `lvasgn`.
fn reassigns_block_arg(def_node: &Node<'_>, name: &[u8]) -> bool {
    let mut found = false;
    each_descendant(def_node, &mut |d: &Node<'_>| {
        let hit = if let Some(n) = d.as_local_variable_write_node() {
            n.name().as_slice() == name
        } else if let Some(n) = d.as_local_variable_or_write_node() {
            n.name().as_slice() == name
        } else if let Some(n) = d.as_local_variable_and_write_node() {
            n.name().as_slice() == name
        } else if let Some(n) = d.as_local_variable_operator_write_node() {
            n.name().as_slice() == name
        } else if let Some(m) = d.as_multi_write_node() {
            m.lefts().iter().any(|n| target_assigns(&n, name))
                || m.rest().is_some_and(|n| target_assigns(&n, name))
                || m.rights().iter().any(|n| target_assigns(&n, name))
        } else if let Some(r) = d.as_rescue_node() {
            r.reference().is_some_and(|n| target_assigns(&n, name))
        } else if let Some(f) = d.as_for_node() {
            target_assigns(&f.index(), name)
        } else {
            false
        };
        if hit {
            found = true;
        }
    });
    found
}

/// Identifies unnecessary use of a `block_given?` where explicit check of
/// block argument would suffice.
#[derive(Debug, Clone)]
pub struct BlockGivenWithExplicitBlock {
    defs: Vec<DefBlockArg>,
}

impl Rule for BlockGivenWithExplicitBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/BlockGivenWithExplicitBlock",
        department: Department::Performance,
        summary: "Check block argument explicitly instead of using `block_given?`.",
        explanation: "Identifies unnecessary use of a `block_given?` where explicit check of block argument would suffice.\n\nNOTE: This cop produces code with significantly worse performance when a block is being passed to the method and as such should not be enabled.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { defs: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(def) = node.as_def_node() {
            let name = def
                .parameters()
                .and_then(|p| p.block())
                .and_then(|b| b.name_loc())
                .map(|loc| ctx.text(ruby_ast::LocationExt::span(&loc)).to_vec())
                .filter(|name| !reassigns_block_arg(node, name));
            self.defs.push(name);
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"block_given?" {
            return;
        }
        let Some(Some(block_arg_name)) = self.defs.last() else { return };
        let span = call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, block_arg_name.clone())],
            },
        );
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_def_node().is_some() {
            self.defs.pop();
        }
    }
}

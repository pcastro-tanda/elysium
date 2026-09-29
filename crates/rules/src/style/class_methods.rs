//! `Style/ClassMethods`, ported from RuboCop's
//! `lib/rubocop/cop/style/class_methods.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};

/// Checks for uses of the class/module name instead of self, when defining
/// class/module methods.
#[derive(Debug, Clone)]
pub struct ClassMethods;

impl Rule for ClassMethods {
    const META: RuleMeta = RuleMeta {
        name: "Style/ClassMethods",
        department: Department::Style,
        summary: "Use self when defining module/class methods.",
        explanation: "\
Checks for uses of the class/module name instead of
self, when defining class/module methods.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::ModuleNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (name, body) = match node.kind() {
            NodeKind::ClassNode => {
                let n = node.as_class_node().expect("kind matched");
                (n.constant_path(), n.body())
            }
            NodeKind::ModuleNode => {
                let n = node.as_module_node().expect("kind matched");
                (n.constant_path(), n.body())
            }
            _ => return,
        };
        let Some(body) = body else { return };

        // whitequark unwraps a single-statement body so `defs_type?` and
        // `begin_type? { each_child_node(:defs) }` both reduce to "look at
        // the direct child statements of the body"; Prism always wraps in
        // a `StatementsNode`, so a single iteration covers both branches.
        let Some(statements) = body.as_statements_node() else { return };
        for child in &statements.body() {
            let Some(def) = child.as_def_node() else { continue };
            check_defs(&name, &def, ctx);
        }
    }
}

fn check_defs(name: &Node<'_>, def: &ruby_ast::node::DefNode<'_>, ctx: &mut Context<'_>) {
    // check if the class/module name matches the definee for the defs node
    let Some(receiver) = def.receiver() else { return };
    if ext::const_name(name).is_none() || ext::const_name(name) != ext::const_name(&receiver) {
        return;
    }

    let class_source = String::from_utf8_lossy(ctx.text(name.span())).into_owned();
    let method_name = String::from_utf8_lossy(def.name().as_slice()).into_owned();
    let message = format!("Use `self.{method_name}` instead of `{class_source}.{method_name}`.");

    let name_span = receiver
        .as_constant_path_node()
        .map_or_else(|| receiver.span(), |path| path.name_loc().span());

    ctx.report_with_fix(
        &ClassMethods::META,
        name_span,
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(receiver.span(), b"self".to_vec())],
        },
    );
}

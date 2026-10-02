//! `Style/OneClassPerFile`, ported from RuboCop's
//! `lib/rubocop/cop/style/one_class_per_file.rb`.
//!
//! `top_level_definition?`'s `node.parent&.begin_type? ? node.parent.root? :
//! node.root?` becomes a check of the ancestor stack: a top-level
//! `ClassNode`/`ModuleNode` sits directly inside the program's own
//! `StatementsNode`, i.e. its ancestors are exactly `[ProgramNode,
//! StatementsNode]`. `node.loc.name` (the whole constant-path source, e.g.
//! `Bar::Qux`) is Prism's `constant_path` node's own span; `node.identifier`
//! (for the `AllowedClasses` check) is Prism's `name` field, which is
//! already just the final segment.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not define multiple classes/modules at the top level in a single file.";

/// Checks that each source file defines at most one top-level class or module.
#[derive(Debug, Clone)]
pub struct OneClassPerFile {
    allowed_classes: Vec<String>,
    /// Count of top-level, non-allowed class/module definitions seen so far
    /// in this file.
    seen: u32,
}

impl Rule for OneClassPerFile {
    const META: RuleMeta = RuleMeta {
        name: "Style/OneClassPerFile",
        department: Department::Style,
        summary: "Checks that each source file defines at most one top-level class or module.",
        explanation: "Keeping one class or module per file makes it easier to find and \
            navigate code, and follows the convention used by most Ruby projects.\n\nClasses \
            and modules listed in `AllowedClasses` are not counted toward the limit. This is \
            useful for small ancillary classes like custom exception classes that logically \
            belong with the main class.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::ModuleNode],
        config: &[ConfigOption {
            name: "AllowedClasses",
            default: ConfigDefault::StrList(&[]),
            allowed: &[],
            doc: "Classes/modules not counted toward the one-per-file limit.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allowed_classes: options.str_list("AllowedClasses"), seen: 0 })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.seen = 0;
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (name, constant_path_span) = match node.kind() {
            NodeKind::ClassNode => {
                let c = node.as_class_node().expect("kind matched");
                (c.name(), c.constant_path().span())
            }
            NodeKind::ModuleNode => {
                let m = node.as_module_node().expect("kind matched");
                (m.name(), m.constant_path().span())
            }
            _ => unreachable!("kind matched"),
        };

        if !is_top_level(ctx) {
            return;
        }
        let name = String::from_utf8_lossy(name.as_slice());
        if self.allowed_classes.iter().any(|allowed| allowed == name.as_ref()) {
            return;
        }

        self.seen += 1;
        if self.seen <= 1 {
            return;
        }

        let span = Span::new(node.span().start, constant_path_span.end);
        ctx.report(&Self::META, span, MSG);
    }
}

/// `top_level_definition?`: the node sits directly inside the program's own
/// statement list, i.e. its ancestor stack is exactly `[ProgramNode,
/// StatementsNode]`.
fn is_top_level(ctx: &Context<'_>) -> bool {
    let ancestors = ctx.ancestors();
    ancestors.len() == 2
        && ancestors[0].kind == NodeKind::ProgramNode
        && ancestors[1].kind == NodeKind::StatementsNode
}

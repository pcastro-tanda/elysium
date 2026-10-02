//! `Style/EmptyClassDefinition`, ported from RuboCop's
//! `lib/rubocop/cop/style/empty_class_definition.rb`.
//!
//! The upstream `class_new_assignment` node matcher requires the casgn's
//! value to literally be a `(send (const _ :Class) :new _)` node. A
//! `Class.new(Parent) { ... }` call carries its block as a sibling `block`
//! node around the `send` in whitequark, so the matcher's `:send`-only
//! shape never matches it; in Prism, a block is just another field on the
//! same `CallNode`, so [`call_without_block`] makes that exclusion explicit.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG_CLASS_KEYWORD: &str =
    "Use the `class` keyword instead of `Class.new` to define an empty class.";
const MSG_CLASS_NEW: &str =
    "Use `Class.new` instead of the `class` keyword to define an empty class.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// `class_keyword` and the deprecated `class_definition` alias: both
    /// behave identically upstream (only `on_casgn` fires for either).
    ClassKeyword,
    ClassNew,
}

/// Enforces consistent style for empty class definitions.
#[derive(Debug, Clone)]
pub struct EmptyClassDefinition {
    style: Style,
    allowed_parent_classes: Vec<String>,
}

impl Rule for EmptyClassDefinition {
    const META: RuleMeta = RuleMeta {
        name: "Style/EmptyClassDefinition",
        department: Department::Style,
        summary: "Enforces consistent style for empty class definitions.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ConstantWriteNode, NodeKind::ConstantPathWriteNode, NodeKind::ClassNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("class_keyword"),
                allowed: &["class_keyword", "class_new", "class_definition"],
                doc: "Whether to prefer a standard class definition or `Class.new`.",
            },
            ConfigOption {
                name: "AllowedParentClasses",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Parent classes for which both styles are permitted.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "class_new" => Style::ClassNew,
            _ => Style::ClassKeyword,
        };
        Ok(Self { style, allowed_parent_classes: options.str_list("AllowedParentClasses") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ConstantWriteNode => {
                let write = node.as_constant_write_node().expect("kind matched");
                self.check_casgn(node, write.name_loc().span(), &write.value(), ctx);
            }
            NodeKind::ConstantPathWriteNode => {
                let write = node.as_constant_path_write_node().expect("kind matched");
                self.check_casgn(node, write.target().as_node().span(), &write.value(), ctx);
            }
            NodeKind::ClassNode => self.check_class(node, ctx),
            _ => {}
        }
    }
}

impl EmptyClassDefinition {
    fn check_casgn(
        &self,
        node: &Node<'_>,
        name_span: ruby_source::Span,
        value: &Node<'_>,
        ctx: &mut Context<'_>,
    ) {
        if self.style != Style::ClassKeyword {
            return;
        }
        let Some(call) = class_new_call(value) else { return };
        let Some(args) = call.arguments() else { return };
        let items: Vec<Node<'_>> = args.arguments().iter().collect();
        let [arg] = items.as_slice() else { return };
        if const_short_name(arg).is_none() {
            return;
        }
        let parent_class_name = String::from_utf8_lossy(ctx.text(arg.span())).into_owned();
        if self.allowed_parent_classes.iter().any(|p| p == &parent_class_name) {
            return;
        }

        let indent = " ".repeat(ctx.line_col(node.span().start).column as usize);
        let class_name = String::from_utf8_lossy(ctx.text(name_span)).into_owned();
        let replacement = format!("class {class_name} < {parent_class_name}\n{indent}end");

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            MSG_CLASS_KEYWORD,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
            },
        );
    }

    fn check_class(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.style != Style::ClassNew {
            return;
        }
        let class = node.as_class_node().expect("kind matched");
        let Some(superclass) = class.superclass() else { return };
        if class.body().is_some() {
            return;
        }
        let parent_class_name = String::from_utf8_lossy(ctx.text(superclass.span())).into_owned();
        if self.allowed_parent_classes.iter().any(|p| p == &parent_class_name) {
            return;
        }

        let class_name =
            String::from_utf8_lossy(ctx.text(class.constant_path().span())).into_owned();
        let replacement = format!("{class_name} = Class.new({parent_class_name})");

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            MSG_CLASS_NEW,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
            },
        );
    }
}

/// Upstream's `class_new_assignment` node matcher: a bare (no block)
/// `Class.new(arg)` call with exactly one argument.
fn class_new_call<'pr>(value: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = value.as_call_node()?;
    if call.block().is_some() || call.is_safe_navigation() {
        return None;
    }
    if call.name().as_slice() != b"new" {
        return None;
    }
    let receiver = call.receiver()?;
    match const_short_name(&receiver)?.as_str() {
        "Class" => Some(call),
        _ => None,
    }
}

/// The short (non-namespaced) name of a `ConstantReadNode`/`ConstantPathNode`,
/// or `None` for anything else -- upstream's `arg.const_type?` guard.
fn const_short_name(node: &Node<'_>) -> Option<String> {
    match node.kind() {
        NodeKind::ConstantReadNode => Some(
            String::from_utf8_lossy(node.as_constant_read_node()?.name().as_slice()).into_owned(),
        ),
        NodeKind::ConstantPathNode => Some(
            String::from_utf8_lossy(node.as_constant_path_node()?.name()?.as_slice()).into_owned(),
        ),
        _ => None,
    }
}

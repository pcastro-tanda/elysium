//! `Lint/InheritException`, ported from RuboCop's
//! `lib/rubocop/cop/lint/inherit_exception.rb`.
//!
//! # `on_class` sibling-shadow check via `StatementsNode`
//!
//! Upstream's `inherit_exception_class_with_omitted_namespace?` walks
//! `class_node.left_siblings`, RuboCop-AST's list of statements preceding
//! this one in the enclosing `begin`/body. Prism always wraps a body
//! (program, class/module body, ...) in a [`NodeKind::StatementsNode`] --
//! see `Layout/EmptyLineBetweenDefs`'s module doc for the same observation
//! -- so this rule subscribes to `StatementsNode` directly and scans its own
//! `body()` list in order, tracking whether a `class Exception`/`module
//! Exception` definition has been seen so far; a candidate `class C <
//! Exception` statement is skipped exactly when one has. This reproduces
//! `left_siblings.any?` without any extra tree walk or parent lookup.
//!
//! The omitted-namespace check itself only applies to a *bare* `Exception`
//! superclass (`class_node.parent_class.namespace&.cbase_type?` is only
//! ever `nil`/falsy for a bare constant, since `exception_class?` already
//! restricts `parent_class.const_name` to exactly `"Exception"`, which only
//! a bare `Exception` or an explicit `::Exception` can produce): an
//! explicit `::Exception` (a [`NodeKind::ConstantPathNode`] with no parent)
//! always offends, bare `Exception` (a [`NodeKind::ConstantReadNode`]) is
//! gated on the sibling scan.
//!
//! # `on_send`'s `Class.new(Exception)` has no sibling check
//!
//! Upstream's `on_send` never calls the omitted-namespace helper, so the
//! `Class.new(Exception)` shape is flagged unconditionally, with no
//! sibling-scope awareness at all -- ported as-is.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, ClassNode, StatementsNode};
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`/`PREFERRED_BASE_CLASS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    StandardError,
    RuntimeError,
}

impl Style {
    /// `PREFERRED_BASE_CLASS[style]`.
    const fn preferred_base_class(self) -> &'static str {
        match self {
            Self::StandardError => "StandardError",
            Self::RuntimeError => "RuntimeError",
        }
    }
}

/// Looks for error classes inheriting from `Exception`. Configurable to
/// suggest either `StandardError` (default) or `RuntimeError` instead.
#[derive(Debug, Clone, Copy)]
pub struct InheritException {
    style: Style,
}

impl Rule for InheritException {
    const META: RuleMeta = RuleMeta {
        name: "Lint/InheritException",
        department: Department::Lint,
        summary: "Avoid inheriting from the `Exception` class.",
        explanation: "Looks for error classes inheriting from `Exception`. \
                      It is configurable to suggest using either `StandardError` (default) or \
                      `RuntimeError` instead.\n\n\
                      This cop's autocorrection is unsafe because `rescue` that omit exception \
                      class handle `StandardError` and its subclasses, but not `Exception` and \
                      its subclasses.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode, NodeKind::CallNode],
        config: &[linter::ConfigOption {
            name: "EnforcedStyle",
            default: linter::ConfigDefault::Str("standard_error"),
            allowed: &["standard_error", "runtime_error"],
            doc: "The preferred base class in favour of `Exception`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "runtime_error" => Style::RuntimeError,
            _ => Style::StandardError,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::StatementsNode => {
                let stmts = node.as_statements_node().expect("kind matched");
                self.check_statements(&stmts, ctx);
            }
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                self.check_class_new_call(&call, ctx);
            }
            _ => {}
        }
    }
}

impl InheritException {
    /// RuboCop's `on_class`, scanned over a `StatementsNode`'s own children
    /// so the "left siblings" the omitted-namespace check needs are simply
    /// the elements already walked. See the module doc.
    fn check_statements(self, stmts: &StatementsNode<'_>, ctx: &mut Context<'_>) {
        let mut seen_exception_def = false;
        for statement in &stmts.body() {
            match statement.kind() {
                NodeKind::ClassNode => {
                    let class_node = statement.as_class_node().expect("kind matched");
                    if own_name_is_exception(&class_node) {
                        seen_exception_def = true;
                    }
                    self.check_class(&class_node, seen_exception_def, ctx);
                }
                NodeKind::ModuleNode => {
                    let module_node = statement.as_module_node().expect("kind matched");
                    if ext::const_name(&module_node.constant_path()).as_deref() == Some("Exception")
                    {
                        seen_exception_def = true;
                    }
                }
                _ => {}
            }
        }
    }

    /// RuboCop's `on_class`: `class_node.parent_class` inheriting from
    /// `Exception`, minus a shadowing local `Exception` definition earlier
    /// in the same body (`seen_exception_def`, only consulted for a bare
    /// `Exception` superclass -- see the module doc).
    fn check_class(
        self,
        class_node: &ClassNode<'_>,
        seen_exception_def: bool,
        ctx: &mut Context<'_>,
    ) {
        let Some(superclass) = class_node.superclass() else { return };
        if ext::const_name(&superclass).as_deref() != Some("Exception") {
            return;
        }
        let is_explicit_top_level = superclass.kind() == NodeKind::ConstantPathNode;
        if !is_explicit_top_level && seen_exception_def {
            return;
        }
        self.report(superclass.span(), ctx);
    }

    /// RuboCop's `on_send`/`class_new_call?`: `Class.new(Exception)`, no
    /// sibling-scope awareness (see the module doc).
    fn check_class_new_call(self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if call.name().as_slice() != b"new" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !ext::is_bare_or_toplevel_const(&receiver)
            || ext::const_name(&receiver).as_deref() != Some("Class")
        {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let list = args.arguments();
        if list.len() != 1 {
            return;
        }
        let Some(argument) = list.iter().next() else { return };
        if !ext::is_bare_or_toplevel_const(&argument) {
            return;
        }
        if ext::const_name(&argument).as_deref() != Some("Exception") {
            return;
        }
        self.report(argument.span(), ctx);
    }

    /// RuboCop's `message`/`add_offense ... corrector.replace`.
    fn report(self, span: Span, ctx: &mut Context<'_>) {
        let prefer = self.style.preferred_base_class();
        let message = format!("Inherit from `{prefer}` instead of `Exception`.");
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, prefer.as_bytes().to_vec())],
            },
        );
    }
}

/// Whether a `class` statement's own name (`constant_path`, RuboCop-AST's
/// `identifier`) is `Exception` -- the sibling-shadow condition also
/// applies to `class Exception < ...`, not just `module Exception`.
fn own_name_is_exception(class_node: &ClassNode<'_>) -> bool {
    ext::const_name(&class_node.constant_path()).as_deref() == Some("Exception")
}

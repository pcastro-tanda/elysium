//! `Style/RedundantConstantBase`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_constant_base.rb`.
//!
//! Whitequark's `cbase` is its own zero-width AST node (just the leading
//! `::`); Prism folds it into the outermost [`NodeKind::ConstantPathNode`]
//! of the path it anchors (`parent: None`, per `lint/constant_resolution.rs`'s
//! doc comment), with the `::` itself as that node's own `delimiter_loc`.
//! Prism has no parent pointers, so the whole-tree walk here tracks an
//! explicit ancestor stack (mirroring `lint/useless_method_definition.rs`)
//! to answer `each_ancestor(:class, :module)`.
//!
//! This port skips `AllCops/UseProjectIndex` (`provably_unshadowed?`'s
//! `rubydex`-backed resolution) and `ConstantPathTargetNode` (a leading `::`
//! on one target of a multiple assignment, e.g. `::A, ::B = 1, 2`): no
//! fixture exercises either, and both are noted in `blind_spots`. A leading
//! `::` on a plain assignment target (`::Foo = 1`) is still caught, since
//! Prism represents it as an ordinary `ConstantPathNode` (the write node's
//! own `target` field).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{walk, LocationExt as _, Node, Visitor};
use ruby_source::Span;

const MSG: &str = "Remove redundant `::`.";

/// Avoid redundant `::` prefix on constant.
#[derive(Debug, Clone)]
pub struct RedundantConstantBase {
    lint_constant_resolution_enabled: bool,
}

impl Rule for RedundantConstantBase {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantConstantBase",
        department: Department::Style,
        summary: "Avoid redundant `::` prefix on constant.",
        explanation: "Avoid redundant `::` prefix on a constant.\n\n\
            How Ruby searches constants is a bit complicated, and it can often be difficult to \
            understand from the code whether the `::` is intended or not. Where `Module.nesting` \
            is empty, there is no need to prepend `::`, so it would be nice to consistently \
            avoid such meaningless `::` prefix to avoid confusion.\n\n\
            NOTE: This cop is disabled if `Lint/ConstantResolution` cop is enabled, \
            to prevent conflicting rules. This is because it respects user configurations \
            that want to enable `Lint/ConstantResolution` cop which is disabled by default.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "Does not implement `AllCops/UseProjectIndex`'s `provably_unshadowed?` \
            (rubydex-backed resolution), nor a leading `::` on one target of a multiple \
            assignment (`ConstantPathTargetNode`); no fixture exercises either.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let lint_constant_resolution_enabled = options
            .peer("Lint/ConstantResolution", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(false);
        Ok(Self { lint_constant_resolution_enabled })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        if self.lint_constant_resolution_enabled {
            return;
        }
        let root = ctx.parsed().root();
        let mut finder = Finder { stack: Vec::new(), offenses: Vec::new() };
        walk(&root, &mut finder);
        for span in finder.offenses {
            ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] },
            );
        }
    }
}

/// Walks the whole tree tracking a live ancestor stack (Prism has no parent
/// pointers), collecting one offense span per redundant `cbase`.
struct Finder<'pr> {
    stack: Vec<Node<'pr>>,
    offenses: Vec<Span>,
}

impl<'pr> Visitor<'pr> for Finder<'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        if let Some(path) = node.as_constant_path_node() {
            if path.parent().is_none() && is_bad(&self.stack, node.location().span()) {
                self.offenses.push(path.delimiter_loc().span());
            }
        }
        self.stack.push(*node);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

/// RuboCop's `bad?`/`module_nesting_ancestors_of`: true (redundant) when no
/// ancestor `class`/`module` counts as nesting this `cbase` -- a `class`
/// ancestor only counts when `cbase_span` lies outside that class's own
/// superclass expression (`used_in_super_class_part?`'s inverse), since a
/// superclass clause is evaluated in the *enclosing* scope, not the class
/// body being opened.
fn is_bad(stack: &[Node<'_>], cbase_span: Span) -> bool {
    !stack.iter().any(|ancestor| is_nesting_ancestor(ancestor, cbase_span))
}

fn is_nesting_ancestor(ancestor: &Node<'_>, cbase_span: Span) -> bool {
    if let Some(class) = ancestor.as_class_node() {
        !class
            .superclass()
            .is_some_and(|superclass| span_contains(superclass.location().span(), cbase_span))
    } else {
        ancestor.as_module_node().is_some()
    }
}

fn span_contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

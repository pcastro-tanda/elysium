//! `Rails/FindByOrAssignmentMemoization`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/find_by_or_assignment_memoization.rb`.

use std::collections::HashSet;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::InstanceVariableOrWriteNode;
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Avoid memoizing `find_by` results with `||=`.";

/// Avoid memoizing `find_by` results with `||=`.
#[derive(Debug, Clone, Default)]
pub struct FindByOrAssignmentMemoization {
    /// Names of the instance variables assigned in any `initialize`
    /// (`initialize_methods`), built on first use for each file.
    initialized: Option<HashSet<Vec<u8>>>,
    /// Spans of `def` bodies `on_def` has already looked at; the `on_send`
    /// check of the same `||=` is then a duplicate offense.
    def_bodies: Vec<Span>,
}

impl Rule for FindByOrAssignmentMemoization {
    const META: RuleMeta = RuleMeta {
        name: "Rails/FindByOrAssignmentMemoization",
        department: Department::Rails,
        summary: "Avoid memoizing `find_by` results with `||=`.",
        explanation: "Avoid memoizing `find_by` results with `||=`.\n\nIt is common to see \
                      code that attempts to memoize `find_by` result by `||=`, but `find_by` \
                      may return `nil`, in which case it is not memoized as intended.\n\nNOTE: \
                      Respecting the object shapes introduced in Ruby 3.2, instance variables \
                      used for memoization that are initialized at object creation are \
                      ignored.\n\n```ruby\n# bad - exclusively doing memoization\ndef \
                      current_user\n  @current_user ||= User.find_by(id: \
                      session[:user_id])\nend\n\n# good\ndef current_user\n  return \
                      @current_user if defined?(@current_user)\n\n  @current_user = \
                      User.find_by(id: session[:user_id])\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::DefNode, NodeKind::InstanceVariableOrWriteNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.initialized = None;
        self.def_bodies.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(def) = node.as_def_node() {
            self.on_def(&def, ctx);
        } else if let Some(write) = node.as_instance_variable_or_write_node() {
            self.on_send(&write, ctx);
        }
    }
}

impl FindByOrAssignmentMemoization {
    /// When a method body contains only memoization, the correction can be
    /// more succinct.
    fn on_def(&mut self, def: &ruby_ast::node::DefNode<'_>, ctx: &mut Context<'_>) {
        let Some(body) = def.body() else { return };
        let Some(statements) = body.as_statements_node() else { return };
        let mut statements = statements.body().iter();
        let (Some(body), None) = (statements.next(), statements.next()) else { return };
        let Some(write) = body.as_instance_variable_or_write_node() else { return };
        let Some(find_by) = find_by_value(&write) else { return };
        let body_span = body.span();
        self.def_bodies.push(body_span);
        let name = write.name();
        if self.instance_variable_assigned(name.as_slice(), ctx) {
            return;
        }
        let variable = String::from_utf8_lossy(name.as_slice());
        let replacement = format!(
            "return {variable} if defined?({variable})\n\n{variable} = {}",
            String::from_utf8_lossy(ctx.text(find_by))
        );
        let edit = match def.equal_loc() {
            // `correct_to_regular_method_definition`: the `=` through the
            // body's start becomes a newline, and `end` follows the method.
            Some(equal) => Edit::replace(
                Span::new(equal.span().start, body_span.end),
                format!("\n{replacement}\nend").into_bytes(),
            ),
            None => Edit::replace(body_span, replacement.into_bytes()),
        };
        ctx.report_with_fix(
            &Self::META,
            body_span,
            MSG,
            Fix { applicability: Applicability::Unsafe, edits: vec![edit] },
        );
    }

    fn on_send(&mut self, write: &InstanceVariableOrWriteNode<'_>, ctx: &mut Context<'_>) {
        let span = write.as_node().span();
        if self.def_bodies.contains(&span) {
            return;
        }
        let Some(find_by) = find_by_value(write) else { return };
        if ctx.ancestors().iter().any(|a| matches!(a.kind, NodeKind::IfNode | NodeKind::UnlessNode))
        {
            return;
        }
        let name = write.name();
        if self.instance_variable_assigned(name.as_slice(), ctx) {
            return;
        }
        let variable = String::from_utf8_lossy(name.as_slice());
        let replacement = format!(
            "if defined?({variable})\n  {variable}\nelse\n  {variable} = {}\nend",
            String::from_utf8_lossy(ctx.text(find_by))
        );
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            },
        );
    }

    /// `instance_variable_assigned?`.
    fn instance_variable_assigned(&mut self, name: &[u8], ctx: &Context<'_>) -> bool {
        self.initialized.get_or_insert_with(|| initialized_variables(ctx)).contains(name)
    }
}

/// `(or_asgn (ivasgn _) $(send _ :find_by ...))`: the span of the `find_by`
/// call, which must be a plain `send` (no safe navigation, no literal block).
fn find_by_value(write: &InstanceVariableOrWriteNode<'_>) -> Option<Span> {
    let value = write.value();
    let call = value.as_call_node()?;
    let literal_block = call.block().is_some_and(|b| b.as_block_node().is_some());
    if call.name().as_slice() != b"find_by" || call.is_safe_navigation() || literal_block {
        return None;
    }
    Some(value.span())
}

/// Every instance variable assigned (`ivasgn`) anywhere under a
/// receiver-less `def initialize`.
fn initialized_variables(ctx: &Context<'_>) -> HashSet<Vec<u8>> {
    let mut names = HashSet::new();
    let root = ctx.parsed().root();
    each_descendant(&root, &mut |node| {
        let Some(def) = node.as_def_node() else { return };
        if def.receiver().is_some() || def.name().as_slice() != b"initialize" {
            return;
        }
        each_descendant(node, &mut |inner| {
            let name = match inner.kind() {
                NodeKind::InstanceVariableWriteNode => {
                    inner.as_instance_variable_write_node().map(|n| n.name())
                }
                NodeKind::InstanceVariableOrWriteNode => {
                    inner.as_instance_variable_or_write_node().map(|n| n.name())
                }
                NodeKind::InstanceVariableAndWriteNode => {
                    inner.as_instance_variable_and_write_node().map(|n| n.name())
                }
                NodeKind::InstanceVariableOperatorWriteNode => {
                    inner.as_instance_variable_operator_write_node().map(|n| n.name())
                }
                NodeKind::InstanceVariableTargetNode => {
                    inner.as_instance_variable_target_node().map(|n| n.name())
                }
                _ => None,
            };
            if let Some(name) = name {
                names.insert(name.as_slice().to_vec());
            }
        });
    });
    names
}

//! `Lint/UnusedBlockArgument`, ported from RuboCop's
//! `lib/rubocop/cop/lint/unused_block_argument.rb` plus the shared pieces it
//! leans on: `Lint::UnusedArgument` (`lib/rubocop/cop/mixin/unused_argument.rb`)
//! and `RuboCop::Cop::UnusedArgCorrector`
//! (`lib/rubocop/cop/correctors/unused_arg_corrector.rb`).
//!
//! RuboCop hooks `after_leaving_scope`, so every scope's variables are
//! checked as the `VariableForce` walk pops it. Order does not matter here:
//! this rule collects every unused block argument/block-local variable from
//! every scope in one pass over [`Semantics::variable_ids`] since the
//! fixture harness compares offenses by position, not by report order.
//!
//! Prism's `BlockNode` hangs off its own `CallNode` instead of owning it
//! (unlike whitequark's `block` node, whose first child *is* the call), so
//! `define_method_call?` and "is this block a lambda call" both need the
//! block's enclosing `CallNode`, found via the last entry of
//! [`Semantics::scope_ancestors`] (the immediate parent, since the builder
//! pushes the `CallNode` onto its ancestor stack before walking into its
//! `block` child).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_semantic::{DeclKind, ScopeId, Semantics, Variable, VariableId};
use ruby_source::{Side, Span};

/// `Lint::UnusedBlockArgument`.
#[derive(Debug, Clone)]
pub struct UnusedBlockArgument {
    ignore_empty_blocks: bool,
    allow_unused_keyword_arguments: bool,
}

impl Rule for UnusedBlockArgument {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UnusedBlockArgument",
        department: Department::Lint,
        summary: "Checks for unused block arguments.",
        explanation: "\
Checks for unused block arguments.

```ruby
# bad
do_something do |used, unused|
  puts used
end

do_something do |bar|
  puts :foo
end

define_method(:foo) do |bar|
  puts :baz
end

# good
do_something do |used, _unused|
  puts used
end

do_something do
  puts :foo
end

define_method(:foo) do |_bar|
  puts :baz
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ProgramNode],
        config: &[
            ConfigOption {
                name: "IgnoreEmptyBlocks",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether to ignore block arguments when the block body is empty.",
            },
            ConfigOption {
                name: "AllowUnusedKeywordArguments",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether to allow unused keyword arguments.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            ignore_empty_blocks: options.bool("IgnoreEmptyBlocks"),
            allow_unused_keyword_arguments: options.bool("AllowUnusedKeywordArguments"),
        })
    }

    fn leave(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if node.kind() != NodeKind::ProgramNode {
            return;
        }
        let mut reports: Vec<(Span, String, Option<Fix>)> = Vec::new();
        {
            let semantics = ctx.semantics();
            for id in semantics.variable_ids() {
                if let Some(report) = self.check_variable(semantics, ctx, id) {
                    reports.push(report);
                }
            }
        }
        for (span, message, fix) in reports {
            match fix {
                Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
                None => ctx.report(&Self::META, span, message),
            }
        }
    }
}

impl UnusedBlockArgument {
    /// RuboCop's `UnusedArgument#check_argument` plus this cop's
    /// `check_argument` override.
    fn check_variable(
        &self,
        semantics: &Semantics<'_>,
        ctx: &Context<'_>,
        id: VariableId,
    ) -> Option<(Span, String, Option<Fix>)> {
        let variable = semantics.variable(id);
        let is_block_argument = semantics.is_block_argument(id);

        // `allowed_block?`
        if !is_block_argument {
            return None;
        }
        if self.ignore_empty_blocks && is_empty_block(semantics, variable.scope()) {
            return None;
        }
        // `allowed_keyword_argument?`
        if variable.is_keyword_argument() && self.allow_unused_keyword_arguments {
            return None;
        }
        // `used_block_local?`
        if variable.is_explicit_block_local() && !variable.assignments().is_empty() {
            return None;
        }
        // `UnusedArgument#check_argument`
        if variable.should_be_unused() || variable.referenced() {
            return None;
        }

        let name_span = name_span(variable);
        let message = message(semantics, variable);
        let fix = autocorrect(ctx, variable, name_span);
        Some((name_span, message, fix))
    }
}

/// RuboCop's `empty_block?`: `_send, _args, body = *variable.scope.node;
/// body.nil?`.
fn is_empty_block(semantics: &Semantics<'_>, scope: ScopeId) -> bool {
    semantics.scope(scope).body().is_none()
}

/// The byte range `variable.declaration_node.loc.name` covers: just the
/// identifier, excluding a leading splat/`&`, a trailing default value, or
/// (for a required positional argument, whose whole node already is the
/// name) nothing extra to exclude.
fn name_span(variable: &Variable<'_>) -> Span {
    let node = variable.declaration();
    match variable.decl_kind() {
        DeclKind::RequiredArg
        | DeclKind::BlockLocal
        | DeclKind::Assignment
        | DeclKind::RegexpNamedCapture
        | DeclKind::PatternMatch => node.span(),
        DeclKind::OptionalArg => {
            node.as_optional_parameter_node().expect("decl kind matched").name_loc().span()
        }
        DeclKind::RestArg => node
            .as_rest_parameter_node()
            .and_then(|n| n.name_loc())
            .map_or(node.span(), |loc| loc.span()),
        // Prism's `name_loc` for a keyword parameter includes the trailing
        // `:` (it is the whole `name:` token); whitequark's `loc.name` is
        // just the identifier, so the last byte is trimmed here.
        DeclKind::KeywordArg => without_trailing_colon(
            node.as_required_keyword_parameter_node().expect("decl kind matched").name_loc().span(),
        ),
        DeclKind::OptionalKeywordArg => without_trailing_colon(
            node.as_optional_keyword_parameter_node().expect("decl kind matched").name_loc().span(),
        ),
        DeclKind::KeywordRestArg => node
            .as_keyword_rest_parameter_node()
            .and_then(|n| n.name_loc())
            .map_or(node.span(), |loc| loc.span()),
        DeclKind::BlockArg => node
            .as_block_parameter_node()
            .and_then(|n| n.name_loc())
            .map_or(node.span(), |loc| loc.span()),
    }
}

/// Drops the trailing `:` Prism includes in a keyword parameter's
/// `name_loc`.
fn without_trailing_colon(span: Span) -> Span {
    Span::new(span.start, span.end.saturating_sub(1))
}

/// RuboCop's `UnusedArgument#message`.
fn message(semantics: &Semantics<'_>, variable: &Variable<'_>) -> String {
    let name = String::from_utf8_lossy(variable.name());
    let base = format!("Unused {} - `{name}`.", variable_type(variable));
    if variable.is_explicit_block_local() {
        base
    } else {
        format!("{base} {}", augmentation(semantics, variable))
    }
}

/// RuboCop's `variable_type`.
fn variable_type(variable: &Variable<'_>) -> &'static str {
    if variable.is_explicit_block_local() {
        "block local variable"
    } else {
        "block argument"
    }
}

/// RuboCop's `augment_message`.
fn augmentation(semantics: &Semantics<'_>, variable: &Variable<'_>) -> String {
    let scope_id = variable.scope();
    let all_argument_ids: Vec<VariableId> = semantics
        .scope(scope_id)
        .variables()
        .iter()
        .copied()
        .filter(|&vid| semantics.is_block_argument(vid))
        .collect();
    let none_referenced = all_argument_ids.iter().all(|&vid| !semantics.variable(vid).referenced());

    if is_lambda_scope(semantics, scope_id) {
        message_for_lambda(variable, none_referenced)
    } else {
        message_for_normal_block(
            semantics,
            scope_id,
            variable,
            none_referenced,
            all_argument_ids.len(),
        )
    }
}

/// RuboCop's `message_for_normal_block`.
fn message_for_normal_block(
    semantics: &Semantics<'_>,
    scope_id: ScopeId,
    variable: &Variable<'_>,
    none_referenced: bool,
    argument_count: usize,
) -> String {
    if none_referenced && !is_define_method_call(semantics, scope_id) {
        if argument_count > 1 {
            "You can omit all the arguments if you don't care about them.".to_string()
        } else {
            "You can omit the argument if you don't care about it.".to_string()
        }
    } else {
        message_for_underscore_prefix(variable)
    }
}

/// RuboCop's `message_for_lambda`.
fn message_for_lambda(variable: &Variable<'_>, none_referenced: bool) -> String {
    let base = message_for_underscore_prefix(variable);
    if none_referenced {
        format!(
            "{base} Also consider using a proc without arguments instead of a lambda if you \
             want it to accept any arguments but don't care about them."
        )
    } else {
        base
    }
}

/// RuboCop's `message_for_underscore_prefix`.
fn message_for_underscore_prefix(variable: &Variable<'_>) -> String {
    let name = String::from_utf8_lossy(variable.name());
    format!("If it's necessary, use `_` or `_{name}` as an argument name to indicate that it won't be used.")
}

/// The `CallNode` a block scope's `BlockNode` hangs off, found through the
/// scope's recorded ancestors (the builder pushes the `CallNode` before
/// walking into its `block` child, so it is always the innermost entry).
fn enclosing_call<'pr>(semantics: &Semantics<'pr>, scope_id: ScopeId) -> Option<Node<'pr>> {
    let ancestor = *semantics.scope_ancestors(scope_id).last()?;
    matches!(ancestor, Node::CallNode { .. }).then_some(ancestor)
}

/// RuboCop's `scope.node.lambda?`: a `-> (...) { }` literal, or a block
/// whose call is named `lambda` (regardless of receiver).
fn is_lambda_scope(semantics: &Semantics<'_>, scope_id: ScopeId) -> bool {
    match semantics.scope(scope_id).node() {
        Node::LambdaNode { .. } => true,
        Node::BlockNode { .. } => enclosing_call(semantics, scope_id)
            .and_then(|call| call.as_call_node())
            .is_some_and(|call| call.name().as_slice() == b"lambda"),
        _ => false,
    }
}

/// RuboCop's `define_method_call?`.
fn is_define_method_call(semantics: &Semantics<'_>, scope_id: ScopeId) -> bool {
    enclosing_call(semantics, scope_id)
        .and_then(|call| call.as_call_node())
        .is_some_and(|call| call.name().as_slice() == b"define_method")
}

/// RuboCop's `UnusedArgCorrector.correct`.
fn autocorrect(ctx: &Context<'_>, variable: &Variable<'_>, name_span: Span) -> Option<Fix> {
    match variable.decl_kind() {
        // `return if %i[kwarg kwoptarg].include?(node.type)`
        DeclKind::KeywordArg | DeclKind::OptionalKeywordArg => None,
        DeclKind::BlockArg => Some(remove_block_arg(ctx, variable)),
        _ => {
            let mut text = Vec::with_capacity(variable.name().len() + 1);
            text.push(b'_');
            text.extend_from_slice(variable.name());
            Some(Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(name_span, text)],
            })
        }
    }
}

/// RuboCop's `correct_for_blockarg_type`: removes the whole `&block`
/// parameter, its immediately preceding horizontal/vertical whitespace, and
/// (if now adjacent) a preceding comma.
fn remove_block_arg(ctx: &Context<'_>, variable: &Variable<'_>) -> Fix {
    let node_span = variable.declaration().span();
    let with_space = ctx.with_surrounding_space(node_span, Side::Left, true, false);
    let start = extend_over_comma_left(ctx, with_space.start);
    Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::delete(Span::new(start, node_span.end))],
    }
}

/// RuboCop's `range_with_surrounding_comma(range, :left)`.
fn extend_over_comma_left(ctx: &Context<'_>, mut start: u32) -> u32 {
    let bytes = ctx.source().bytes();
    while start > 0 && bytes[(start - 1) as usize] == b',' {
        start -= 1;
    }
    start
}

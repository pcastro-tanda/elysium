//! `Lint/UnusedMethodArgument`, ported from RuboCop's
//! `lib/rubocop/cop/lint/unused_method_argument.rb` plus the pieces it leans
//! on: `lib/rubocop/cop/mixin/unused_argument.rb` (`VariableForce`, here
//! [`ruby_semantic`]) and `lib/rubocop/cop/correctors/unused_arg_corrector.rb`
//! for the autocorrection shapes.
//!
//! RuboCop hooks `after_leaving_scope`, so every scope is checked as the
//! `VariableForce` walk pops it. That order is reproduced by iterating
//! [`ruby_semantic::Semantics::leave_order`] from this rule's single `leave`
//! on the root, the same shape `lint/useless_assignment.rs` uses.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_semantic::{DeclKind, Semantics, VariableId};
use ruby_source::Span;
use std::fmt::Write as _;

/// `Lint::UnusedMethodArgument`.
#[derive(Debug, Clone)]
pub struct UnusedMethodArgument {
    allow_unused_keyword_arguments: bool,
    ignore_empty_methods: bool,
    ignore_not_implemented_methods: bool,
    not_implemented_exceptions: Vec<String>,
}

impl Rule for UnusedMethodArgument {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UnusedMethodArgument",
        department: Department::Lint,
        summary: "Checks for unused method arguments.",
        explanation: "\
Checks for unused method arguments.

```ruby
# bad
def some_method(used, unused, _unused_but_allowed)
  puts used
end

# good
def some_method(used, _unused, _unused_but_allowed)
  puts used
end
```

With `AllowUnusedKeywordArguments: false` (the default), an unused keyword
argument is still flagged, just without the `_foo` rename suggestion.
With `AllowUnusedKeywordArguments: true`, unused keyword arguments are
accepted outright.

With `IgnoreEmptyMethods: true` (the default), a method with an empty body
is not flagged, even if it declares unused arguments. With
`IgnoreEmptyMethods: false`, empty methods are flagged too.

With `IgnoreNotImplementedMethods: true` (the default), a method whose
entire body is `raise NotImplementedError` (or any class named in
`NotImplementedExceptions`) or `fail ...` is not flagged. With
`IgnoreNotImplementedMethods: false`, such methods are flagged too.

A block argument that is never read is still accepted when the method's
body (or any nested block) contains a bare `yield`, since the argument is
there only to document that the method yields.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ProgramNode],
        config: &[
            ConfigOption {
                name: "AllowUnusedKeywordArguments",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether unused keyword arguments are accepted, not just exempted from the `_foo` rename suggestion.",
            },
            ConfigOption {
                name: "IgnoreEmptyMethods",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether to accept an unused argument in a method with an empty body.",
            },
            ConfigOption {
                name: "IgnoreNotImplementedMethods",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether to accept an unused argument in a method whose body only raises or fails.",
            },
            ConfigOption {
                name: "NotImplementedExceptions",
                default: ConfigDefault::StrList(&["NotImplementedError"]),
                allowed: &[],
                doc: "Exception class names that count as \"not implemented\" for `IgnoreNotImplementedMethods`.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_unused_keyword_arguments: options.bool("AllowUnusedKeywordArguments"),
            ignore_empty_methods: options.bool("IgnoreEmptyMethods"),
            ignore_not_implemented_methods: options.bool("IgnoreNotImplementedMethods"),
            not_implemented_exceptions: options.str_list("NotImplementedExceptions"),
        })
    }

    fn leave(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if node.kind() != NodeKind::ProgramNode {
            return;
        }
        let mut reports: Vec<(Span, String, Option<Fix>)> = Vec::new();
        {
            let semantics = ctx.semantics();
            for &scope in semantics.leave_order() {
                for &variable in semantics.scope(scope).variables() {
                    self.check_argument(semantics, ctx, variable, &mut reports);
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

impl UnusedMethodArgument {
    /// RuboCop's `UnusedMethodArgument#check_argument` plus
    /// `UnusedArgument#check_argument` (the `super` call).
    fn check_argument(
        &self,
        semantics: &Semantics<'_>,
        ctx: &Context<'_>,
        id: VariableId,
        reports: &mut Vec<(Span, String, Option<Fix>)>,
    ) {
        if !semantics.is_method_argument(id) {
            return;
        }
        let variable = semantics.variable(id);
        if variable.is_keyword_argument() && self.allow_unused_keyword_arguments {
            return;
        }
        let scope = semantics.scope(variable.scope());
        let body = scope.body();
        if self.ignore_empty_methods && body.is_none() {
            return;
        }
        if self.ignore_not_implemented_methods
            && body.is_some_and(|body| is_not_implemented(&body, &self.not_implemented_exceptions))
        {
            return;
        }
        if block_argument_with_yield(semantics, id) {
            return;
        }
        // `UnusedArgument#check_argument`.
        if variable.should_be_unused() || variable.referenced() {
            return;
        }

        let declaration = variable.declaration();
        let name = variable.name();
        let name_span = argument_name_span(&declaration, variable.decl_kind());
        let message =
            build_message(semantics, variable.scope(), name, variable.is_keyword_argument());
        let fix = autocorrect(ctx, &declaration, variable.decl_kind(), name, name_span);
        reports.push((name_span, message, fix));
    }
}

/// RuboCop's `UnusedMethodArgument#message`.
fn build_message(
    semantics: &Semantics<'_>,
    scope: ruby_semantic::ScopeId,
    name: &[u8],
    is_keyword_argument: bool,
) -> String {
    let name = String::from_utf8_lossy(name);
    let mut message = format!("Unused method argument - `{name}`.");
    if !is_keyword_argument {
        write!(
            message,
            " If it's necessary, use `_` or `_{name}` as an argument name to indicate that it \
             won't be used. If it's unnecessary, remove it."
        )
        .expect("String write is infallible");
    }

    let scope = semantics.scope(scope);
    let all_referenced = scope
        .variables()
        .iter()
        .filter(|&&id| semantics.is_method_argument(id))
        .any(|&id| semantics.variable(id).referenced());
    if !all_referenced {
        let scope_name = scope
            .node()
            .as_def_node()
            .map(|def| String::from_utf8_lossy(def.name().as_slice()).into_owned())
            .unwrap_or_default();
        write!(
            message,
            " You can also write as `{scope_name}(*)` if you want the method to accept any \
             arguments but don't care about them."
        )
        .expect("String write is infallible");
    }

    message
}

/// RuboCop's `UnusedMethodArgument#block_argument_with_yield?`.
fn block_argument_with_yield(semantics: &Semantics<'_>, id: VariableId) -> bool {
    let variable = semantics.variable(id);
    if variable.decl_kind() != DeclKind::BlockArg {
        return false;
    }
    let Some(body) = semantics.scope(variable.scope()).body() else { return false };
    let mut found = false;
    each_descendant(&body, &mut |n| {
        if n.kind() == NodeKind::YieldNode {
            found = true;
        }
    });
    found
}

/// RuboCop's `UnusedMethodArgument#not_implemented?`: `{(send nil? :raise
/// #allowed_exception_class? ...) (send nil? :fail ...)}`. whitequark elides
/// a single-statement method body's wrapper, so only a `StatementsNode` with
/// exactly one statement (or a body that already is one, never the case for
/// a Prism `def`'s body) can match.
fn is_not_implemented(body: &Node<'_>, allowed_exceptions: &[String]) -> bool {
    let statement = match body.kind() {
        NodeKind::StatementsNode => {
            let statements = body.as_statements_node().expect("kind matched").body();
            if statements.len() != 1 {
                return false;
            }
            statements.first().expect("length checked")
        }
        _ => *body,
    };
    let Some(call) = statement.as_call_node() else { return false };
    if call.receiver().is_some() {
        return false;
    }
    match call.name().as_slice() {
        b"raise" => {
            let Some(first_arg) = call.arguments().and_then(|args| args.arguments().iter().next())
            else {
                return false;
            };
            matches!(first_arg.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
                && const_name(&first_arg).is_some_and(|name| allowed_exceptions.contains(&name))
        }
        b"fail" => true,
        _ => false,
    }
}

/// RuboCop's `variable.declaration_node.loc.name`: the identifier alone, not
/// the `*`/`&` operator, `= default`, or `:`/`: default` a parameter node's
/// own span would include.
fn argument_name_span(node: &Node<'_>, decl_kind: DeclKind) -> Span {
    match decl_kind {
        DeclKind::OptionalArg => {
            node.as_optional_parameter_node().expect("kind matched").name_loc().span()
        }
        DeclKind::RestArg => node
            .as_rest_parameter_node()
            .expect("kind matched")
            .name_loc()
            .map_or_else(|| node.span(), |loc| loc.span()),
        DeclKind::KeywordArg => {
            let span =
                node.as_required_keyword_parameter_node().expect("kind matched").name_loc().span();
            Span::new(span.start, span.end - 1)
        }
        DeclKind::OptionalKeywordArg => {
            let span =
                node.as_optional_keyword_parameter_node().expect("kind matched").name_loc().span();
            Span::new(span.start, span.end - 1)
        }
        DeclKind::KeywordRestArg => node
            .as_keyword_rest_parameter_node()
            .expect("kind matched")
            .name_loc()
            .map_or_else(|| node.span(), |loc| loc.span()),
        DeclKind::BlockArg => node
            .as_block_parameter_node()
            .expect("kind matched")
            .name_loc()
            .map_or_else(|| node.span(), |loc| loc.span()),
        DeclKind::RequiredArg => node.span(),
        DeclKind::BlockLocal
        | DeclKind::Assignment
        | DeclKind::RegexpNamedCapture
        | DeclKind::PatternMatch => unreachable!("not an argument kind"),
    }
}

/// RuboCop's `UnusedArgCorrector.correct`.
fn autocorrect(
    ctx: &Context<'_>,
    declaration: &Node<'_>,
    decl_kind: DeclKind,
    name: &[u8],
    name_span: Span,
) -> Option<Fix> {
    match decl_kind {
        DeclKind::KeywordArg | DeclKind::OptionalKeywordArg => None,
        DeclKind::BlockArg => {
            let span = block_arg_removal_span(ctx, declaration.span());
            Some(Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] })
        }
        _ => {
            let mut replacement = Vec::with_capacity(name.len() + 1);
            replacement.push(b'_');
            replacement.extend_from_slice(name);
            Some(Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(name_span, replacement)],
            })
        }
    }
}

/// RuboCop's `UnusedArgCorrector.correct_for_blockarg_type`:
/// `range_with_surrounding_space(node.source_range, side: :left)` then
/// `range_with_surrounding_comma(range, :left)`.
fn block_arg_removal_span(ctx: &Context<'_>, node_span: Span) -> Span {
    let bytes = ctx.source().bytes();
    let mut begin = node_span.start as usize;
    while begin > 0 && matches!(bytes[begin - 1], b' ' | b'\t') {
        begin -= 1;
    }
    while begin > 0 && bytes[begin - 1] == b'\n' {
        begin -= 1;
    }
    while begin > 0 && bytes[begin - 1] == b',' {
        begin -= 1;
    }
    Span::new(u32::try_from(begin).expect("offset exceeds u32"), node_span.end)
}

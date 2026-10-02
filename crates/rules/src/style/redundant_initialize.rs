//! `Style/RedundantInitialize`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_initialize.rb`.
//!
//! Upstream's `initialize_forwards?` node matcher requires the parameter
//! list to be *only* plain required `arg`s (`(args $arg*)`) before even
//! looking at the body; Prism's `ParametersNode` exposes each parameter
//! kind as its own field, so this port checks `optionals`/`posts`/
//! `keywords`/`keyword_rest`/`rest`/`block` are all empty/absent directly,
//! rather than node-matching every child's type. A single-statement body
//! is Prism's `StatementsNode` with exactly one element (unwrapped to that
//! element, matching whitequark's elision of a lone statement); `super`
//! with any explicit argument list (including empty parens) is Prism's
//! `SuperNode`, while bare `super` (auto-forwarding, whitequark's `zsuper`)
//! is the dedicated `ForwardingSuperNode`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ParametersNode, SuperNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Remove unnecessary `initialize` method.";
/// RuboCop's `MSG_EMPTY`.
const MSG_EMPTY: &str = "Remove unnecessary empty `initialize` method.";

/// Checks for redundant `initialize` methods.
#[derive(Debug, Clone)]
pub struct RedundantInitialize {
    allow_comments: bool,
}

impl Rule for RedundantInitialize {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantInitialize",
        department: Department::Style,
        summary: "Checks for redundant `initialize` methods.",
        explanation: "\
An initializer is redundant if it does not do anything, or if it only
calls `super` with the same arguments given to it. If the initializer takes
an argument that accepts multiple values (`restarg`, `kwrestarg`, etc.) it
will not register an offense, because it allows the initializer to take a different
number of arguments as its superclass potentially does.

NOTE: If an initializer takes any arguments and has an empty body, RuboCop
assumes it to *not* be redundant. This is to prevent potential `ArgumentError`.

NOTE: If an initializer argument has a default value, RuboCop assumes it
to *not* be redundant.

NOTE: Empty initializers are registered as offenses, but it is possible
to purposely create an empty `initialize` method to override a superclass's
initializer.

```ruby
# bad
def initialize
end

# bad
def initialize
  super
end

# bad
def initialize(a, b)
  super
end

# bad
def initialize(a, b)
  super(a, b)
end

# good
def initialize
  do_something
end

# good
def initialize
  do_something
  super
end

# good (different number of parameters)
def initialize(a, b)
  super(a)
end

# good (default value)
def initialize(a, b = 5)
  super
end

# good (default value)
def initialize(a, b: 5)
  super
end

# good (changes the parameter requirements)
def initialize(_)
end

# good (changes the parameter requirements)
def initialize(*)
end

# good (changes the parameter requirements)
def initialize(**)
end

# good (changes the parameter requirements)
def initialize(...)
end
```

With `AllowComments: true` (the default), a comment anywhere in the method
body is enough to keep it from being flagged.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[linter::ConfigOption {
            name: "AllowComments",
            default: linter::ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Allow comments in the method body to prevent the method from being removed.",
        }],
        blind_spots: "\
`comments_contain_disables?` (an in-body `# rubocop:disable` comment still
counting against `AllowComments`) is not replicated: any comment anywhere
in the method's span keeps it from being flagged, regardless of whether
that comment is itself a disable directive.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_comments: options.bool("AllowComments") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        if def.name().as_slice() != b"initialize" {
            return;
        }
        if forwards(def.parameters().as_ref()) {
            return;
        }
        if self.allow_comments && has_comment(ctx, node.span()) {
            return;
        }

        match def.body() {
            None => {
                if params_empty(def.parameters().as_ref()) {
                    register(ctx, node, MSG_EMPTY);
                }
            }
            Some(body) => {
                let Some(stmt) = single_statement(&body) else { return };
                if let Some(super_node) = stmt.as_super_node() {
                    if same_args(&super_node, def.parameters().as_ref()) {
                        register(ctx, node, MSG);
                    }
                } else if stmt.as_forwarding_super_node().is_some()
                    && required_only(def.parameters().as_ref())
                {
                    register(ctx, node, MSG);
                }
            }
        }
    }
}

/// RuboCop's `register_offense`.
fn register(ctx: &mut Context<'_>, node: &Node<'_>, message: &'static str) {
    let deleted = ctx.whole_lines(node.span());
    ctx.report_with_fix(
        &RedundantInitialize::META,
        node.span(),
        message,
        Fix { applicability: Applicability::Unsafe, edits: vec![Edit::delete(deleted)] },
    );
}

/// RuboCop's `forwards?`: any `restarg`/`kwrestarg`/`forward_args`/
/// `forward_arg` parameter -- Prism's `rest`/`keyword_rest` fields (the
/// latter covering both `**`/`**kwargs` and `...`, see the module doc).
fn forwards(params: Option<&ParametersNode<'_>>) -> bool {
    let Some(params) = params else { return false };
    params.rest().is_some() || params.keyword_rest().is_some()
}

/// `node.arguments.empty?` -- no parameter list at all, or an empty one.
fn params_empty(params: Option<&ParametersNode<'_>>) -> bool {
    let Some(params) = params else { return true };
    params.requireds().iter().count() == 0
        && params.optionals().iter().count() == 0
        && params.posts().iter().count() == 0
        && params.keywords().iter().count() == 0
        && params.rest().is_none()
        && params.keyword_rest().is_none()
        && params.block().is_none()
}

/// The parameter list is empty or made up only of plain required
/// positional parameters -- RuboCop's `(args $arg*)` pattern.
fn required_only(params: Option<&ParametersNode<'_>>) -> bool {
    let Some(params) = params else { return true };
    params.optionals().iter().count() == 0
        && params.posts().iter().count() == 0
        && params.keywords().iter().count() == 0
        && params.rest().is_none()
        && params.keyword_rest().is_none()
        && params.block().is_none()
}

/// The body's sole statement, unwrapping Prism's `StatementsNode` (always
/// present for a non-empty body, even a single statement) -- `None` for an
/// empty, multi-statement, or otherwise-shaped body (e.g. a `rescue`'s
/// `BeginNode`), matching upstream's `node.body.begin_type?` early return.
fn single_statement<'pr>(body: &Node<'pr>) -> Option<Node<'pr>> {
    let stmts = body.as_statements_node()?;
    let mut iter = stmts.body().iter();
    let first = iter.next()?;
    if iter.next().is_some() {
        return None;
    }
    Some(first)
}

/// RuboCop's `same_args?`: an explicit `super(...)`'s argument list must be
/// required-only parameters (the pattern gate) and, positionally, each
/// argument a bare reference to the same-named parameter.
fn same_args(super_node: &SuperNode<'_>, params: Option<&ParametersNode<'_>>) -> bool {
    if !required_only(params) {
        return false;
    }
    let param_names: Vec<&[u8]> = params
        .map(|p| p.requireds().iter().filter_map(|n| param_name(&n)).collect())
        .unwrap_or_default();
    let args: Vec<Node<'_>> =
        super_node.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
    if param_names.len() != args.len() {
        return false;
    }
    param_names.iter().zip(args.iter()).all(|(name, arg)| {
        arg.as_local_variable_read_node().is_some_and(|lvar| lvar.name().as_slice() == *name)
    })
}

/// A `RequiredParameterNode`'s name.
fn param_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    node.as_required_parameter_node().map(|p| p.name().as_slice())
}

/// Any comment within `span` -- RuboCop's `contains_comments?`.
fn has_comment(ctx: &Context<'_>, span: ruby_source::Span) -> bool {
    ctx.comments().iter().any(|c| span.contains(c.span))
}

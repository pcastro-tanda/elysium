//! `Style/SingleLineMethods`, ported from RuboCop's
//! `lib/rubocop/cop/style/single_line_methods.rb`, plus the
//! `LineBreakCorrector` autocorrection helper it uses directly
//! (`lib/rubocop/cop/correctors/line_break_corrector.rb`).
//!
//! Prism always wraps a `def`'s body in a [`NodeKind::StatementsNode`], even
//! a single-statement one (whitequark elides that wrapper). This port reads
//! the body's direct children as "parts" uniformly for both the
//! multi-statement case (each part gets its own line) and the
//! single-statement case (one part, so one line) -- upstream's separate
//! `body.begin_type? && body.parenthesized_call?` branch (an explicitly
//! parenthesized body, e.g. `def foo() (x) end`) collapses into the same
//! code path here, since a `(x)` body is Prism's [`NodeKind::ParenthesesNode`]
//! and is still exactly one part either way. A method body containing a
//! top-level `rescue`/`ensure` is a [`NodeKind::BeginNode`] directly (no
//! `StatementsNode` wrapper at all, matching whitequark's `:kwbegin`); that
//! case is treated as a single opaque part too.
//!
//! `correct_to_endless?` is ported as [`endless_eligible`]: it needs the
//! def's own `assignment_method?` (name ends with `=`, and is not a
//! comparison operator) and the single body part's kind (blocking
//! `if`/`unless`/`while`/`until`, `return`/`break`/`next`, and the
//! `ParenthesesNode`/`BeginNode` "still begin-shaped" cases above).
//!
//! `Style/EndlessMethod`'s `Enabled`/`EnforcedStyle` are read as peer
//! options at `configure` time (peer options are per-file config, not
//! per-node, so they are resolved once and stashed on `self`, like every
//! other peer-reading rule in this crate). `Enabled` is `pending` in
//! `config/default.yml`, which this engine's `LoadedConfig` resolves to a
//! plain `false` unless a project turns `NewCops` on, matching
//! `cop_enabled?` for an unconfigured pending cop.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, DefNode, ParametersNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Avoid single-line method definitions.";

/// RuboCop's `MethodDispatchNode::ARITHMETIC_OPERATORS`.
fn is_arithmetic_operator(name: &[u8]) -> bool {
    matches!(name, b"+" | b"-" | b"*" | b"/" | b"%" | b"**")
}

/// `rubocop-ast`'s `Node::COMPARISON_OPERATORS`.
fn is_comparison_operator(name: &[u8]) -> bool {
    matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=" | b">" | b"<")
}

/// `MethodIdentifierPredicates#assignment_method?`: ends with `=`, but not a
/// comparison operator (`!=`/`<=`/`>=`/`===` also end with `=`-shaped
/// bytes, and must not count).
fn is_assignment_method(name: &[u8]) -> bool {
    !is_comparison_operator(name) && name.ends_with(b"=")
}

/// `rubocop-ast`'s `Node::BASIC_CONDITIONALS` (`if`/`while`/`until`, which
/// also covers `unless` and a full ternary -- both parse as `if` upstream).
fn is_basic_conditional(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::IfNode | NodeKind::UnlessNode | NodeKind::WhileNode | NodeKind::UntilNode
    )
}

/// RuboCop's `NOT_SUPPORTED_ENDLESS_METHOD_BODY_TYPES`.
fn is_unsupported_endless_body(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::ReturnNode | NodeKind::BreakNode | NodeKind::NextNode)
}

/// Still "begin-shaped" the way whitequark's `type?(:begin, :kwbegin)` would
/// see it: an explicitly parenthesized body, or the implicit
/// `rescue`/`ensure` wrapper.
fn is_begin_shaped(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::ParenthesesNode | NodeKind::BeginNode)
}

/// The def body's direct children, read uniformly whether Prism gave a
/// `StatementsNode` (the common case) or something else entirely (the
/// implicit-`rescue` `BeginNode` case, passed through as its own single
/// "part").
fn body_parts<'a>(body: &Node<'a>) -> Vec<Node<'a>> {
    match body.as_statements_node() {
        Some(stmts) => stmts.body().iter().collect(),
        None => vec![*body],
    }
}

/// `ParameterizedNode#arguments.any?`: whether a parameter list has at least
/// one entry of any kind.
fn params_any(params: &ParametersNode<'_>) -> bool {
    !params.requireds().is_empty()
        || !params.optionals().is_empty()
        || params.rest().is_some()
        || !params.posts().is_empty()
        || !params.keywords().is_empty()
        || params.keyword_rest().is_some()
        || params.block().is_some()
}

/// RuboCop's `correct_to_endless?`.
fn endless_eligible(
    def: &DefNode<'_>,
    parts: &[Node<'_>],
    target_ruby_version: f32,
    disallow_endless_style: bool,
) -> bool {
    if target_ruby_version < 3.0 || disallow_endless_style {
        return false;
    }
    let [body] = parts else { return false };
    if is_begin_shaped(body.kind()) || is_basic_conditional(body.kind()) {
        return false;
    }
    if is_assignment_method(def.name().as_slice()) {
        return false;
    }
    !is_unsupported_endless_body(body.kind())
}

/// RuboCop's `require_parentheses?`.
fn require_parentheses(call: &CallNode<'_>) -> bool {
    if is_arithmetic_operator(call.name().as_slice()) {
        return false;
    }
    let has_args = call.arguments().is_some_and(|args| !args.arguments().is_empty());
    has_args && !is_comparison_operator(call.name().as_slice())
}

/// RuboCop's `method_body_source`.
fn method_body_source(ctx: &Context<'_>, body: &Node<'_>) -> String {
    let Some(call) = body.as_call_node() else {
        return String::from_utf8_lossy(ctx.text(body.span())).into_owned();
    };
    if !require_parentheses(&call) {
        return String::from_utf8_lossy(ctx.text(body.span())).into_owned();
    }
    let joined = call
        .arguments()
        .expect("require_parentheses checked non-empty arguments")
        .arguments()
        .iter()
        .map(|a| String::from_utf8_lossy(ctx.text(a.span())).into_owned())
        .collect::<Vec<_>>()
        .join(", ");
    let base = format!("{}({joined})", String::from_utf8_lossy(call.name().as_slice()));
    match call.receiver() {
        Some(receiver) => format!("{}.{base}", String::from_utf8_lossy(ctx.text(receiver.span()))),
        None => base,
    }
}

/// RuboCop's `correct_to_endless`.
fn correct_to_endless(ctx: &Context<'_>, def: &DefNode<'_>, body: &Node<'_>) -> String {
    let receiver = def
        .receiver()
        .map(|r| format!("{}.", String::from_utf8_lossy(ctx.text(r.span()))))
        .unwrap_or_default();
    let args = match def.parameters() {
        Some(params) if params_any(&params) => match (def.lparen_loc(), def.rparen_loc()) {
            (Some(l), Some(r)) => {
                String::from_utf8_lossy(ctx.text(Span::new(l.span().start, r.span().end)))
                    .into_owned()
            }
            _ => String::from_utf8_lossy(ctx.text(params.location().span())).into_owned(),
        },
        _ => "()".to_string(),
    };
    let body_source = method_body_source(ctx, body);
    let name = String::from_utf8_lossy(def.name().as_slice());
    format!("def {receiver}{name}{args} = {body_source}")
}

/// RuboCop's `correct_to_multiline` + `move_comment`, built as one `Fix`'s
/// edits: `LineBreakCorrector.break_line_before` inserts `"\n" + indent`
/// right before a range's start, touching no existing whitespace; moving an
/// end-of-line comment deletes just the comment's own span (the space
/// before it, once orphaned, is left in place -- matching upstream, whose
/// fixtures show it as trailing whitespace) and inserts the comment text
/// plus a newline and the def's own indentation right before the def
/// keyword.
fn correct_to_multiline(ctx: &Context<'_>, def: &DefNode<'_>, parts: &[Node<'_>]) -> Fix {
    let base_indent = ctx.line_col(def.def_keyword_loc().span().start).column;
    let width = 2u32;
    let mut edits = Vec::with_capacity(parts.len() + 3);
    let body_indent = " ".repeat((base_indent + width) as usize);
    for part in parts {
        edits.push(Edit::insert(part.span().start, format!("\n{body_indent}").into_bytes()));
    }
    let end_indent = " ".repeat(base_indent as usize);
    let end_span = def
        .end_keyword_loc()
        .expect("non-endless def has an end keyword, checked by caller")
        .span();
    edits.push(Edit::insert(end_span.start, format!("\n{end_indent}").into_bytes()));

    let node_span = def.location().span();
    let start_line = ctx.line_col(node_span.start).line;
    if let Some(comment) = ctx.comments().iter().find(|c| c.line == start_line) {
        let text = String::from_utf8_lossy(ctx.text(comment.span)).into_owned();
        edits.push(Edit::insert(node_span.start, format!("{text}\n{end_indent}").into_bytes()));
        edits.push(Edit::delete(comment.span));
    }
    Fix { applicability: Applicability::Safe, edits }
}

/// Avoid single-line methods.
#[derive(Debug, Clone)]
pub struct SingleLineMethods {
    allow_if_method_is_empty: bool,
    /// `disallow_endless_method_style?`: `Style/EndlessMethod` is disabled,
    /// or its `EnforcedStyle` is `disallow`.
    disallow_endless_style: bool,
    target_ruby_version: f32,
}

impl Rule for SingleLineMethods {
    const META: RuleMeta = RuleMeta {
        name: "Style/SingleLineMethods",
        department: Department::Style,
        summary: "Avoid single-line methods.",
        explanation: "\
Checks for single-line method definitions that contain a body. It will \
accept single-line methods with no body.

Endless methods added in Ruby 3.0 are also accepted by this cop.

If `Style/EndlessMethod` is enabled with `EnforcedStyle: allow_single_line`, \
`allow_always`, `require_single_line`, or `require_always`, single-line \
methods will be autocorrected to endless methods if there is only one \
statement in the body.

```ruby
# bad
def some_method; body end
def link_to(url); {:name => url}; end
def @table.columns; super; end

# good
def self.resource_class=(klass); end
def @table.columns; end
def some_method() = body
```

With `AllowIfMethodIsEmpty: false`, a completely empty single-line method \
(`def no_op; end`) is also flagged; the default (`true`) accepts it.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[ConfigOption {
            name: "AllowIfMethodIsEmpty",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Whether a single-line method with no body (`def no_op; end`) is accepted.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let endless_enabled = options
            .peer("Style/EndlessMethod", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(false);
        let endless_style = options
            .peer("Style/EndlessMethod", "EnforcedStyle")
            .and_then(OptionValue::as_str)
            .unwrap_or("allow_single_line");
        Ok(Self {
            allow_if_method_is_empty: options.bool("AllowIfMethodIsEmpty"),
            disallow_endless_style: !endless_enabled || endless_style == "disallow",
            target_ruby_version: options.target_ruby_version(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        let span = def.location().span();
        if !ctx.is_single_line(span) || def.end_keyword_loc().is_none() {
            return;
        }
        let body = def.body();
        if body.is_none() && self.allow_if_method_is_empty {
            return;
        }

        let parts = body.as_ref().map(body_parts).unwrap_or_default();
        let eligible = !parts.is_empty()
            && endless_eligible(
                &def,
                &parts,
                self.target_ruby_version,
                self.disallow_endless_style,
            );

        let fix = if eligible {
            let replacement = correct_to_endless(ctx, &def, &parts[0]);
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            }
        } else {
            correct_to_multiline(ctx, &def, &parts)
        };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}

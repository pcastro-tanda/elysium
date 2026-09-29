//! `Style/FormatString`, ported from RuboCop's
//! `lib/rubocop/cop/style/format_string.rb`.
//!
//! # The three offense shapes
//!
//! Upstream's `formatter` node-matcher recognizes a call as a "format
//! string" use if it is one of:
//! - a receiverless `format`/`sprintf` call with at least two arguments
//!   ([`FormatString::detect`]'s first arm);
//! - `receiver % arg` where `receiver` is a plain or interpolated string
//!   literal (whitequark's `str`/`dstr`, matched here against Prism's
//!   [`NodeKind::StringNode`]/[`NodeKind::InterpolatedStringNode`]), for any
//!   `arg` (second arm); or
//! - `receiver % arg` where `receiver` is anything else but `arg` is itself
//!   an array or (braced or bare-keyword) hash literal -- RuboCop's
//!   fallback heuristic for spotting `%` used as the format operator when it
//!   cannot otherwise tell the receiver's type (third arm).
//!
//! Whichever arm matches decides the "detected style" (`percent` for `%`,
//! else the bare call's own name); no offense is raised when that already
//! equals the configured `EnforcedStyle`.
//!
//! # Autocorrection
//!
//! - `format`/`sprintf` <-> each other: renaming the selector text is always
//!   safe (identical argument lists).
//! - `format`/`sprintf` -> `%` ([`FormatString::autocorrect_to_percent`]):
//!   the format string becomes the `%` receiver; a single remaining
//!   argument becomes the RHS directly (unwrapping a splat to its
//!   expression, parenthesizing an RHS that would otherwise bind looser than
//!   `%`, and braces around a bare/braced hash so it stays one argument);
//!   more than one becomes an explicit `[...]` array.
//! - `%` -> `format`/`sprintf` ([`FormatString::autocorrect_from_percent`]):
//!   an array or hash RHS is splatted back into positional/keyword
//!   arguments (each element's own source, comma-joined); anything else
//!   passes through as the sole second argument. This step is skipped
//!   (offense reported, no fix attached) when that RHS is
//!   [`is_ambiguous_percent_argument`]: a bare local-variable read, or a
//!   call to any method but the known scalar-returning conversions in
//!   [`is_known_conversion_method`] -- RuboCop cannot otherwise rule out the
//!   value being an `Array`, whose elements `String#%` already spreads
//!   across the format string's placeholders but a positional `format`/
//!   `sprintf` argument would not.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, NodeList};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Format,
    Sprintf,
    Percent,
}

impl Style {
    /// RuboCop's `method_name`: how a style names itself in the offense
    /// message.
    fn display(self) -> &'static str {
        match self {
            Style::Format => "format",
            Style::Sprintf => "sprintf",
            Style::Percent => "String#%",
        }
    }

    /// The bare method name a `format`/`sprintf` call is renamed to; never
    /// invoked for [`Style::Percent`] (that conversion goes through
    /// [`FormatString::autocorrect_to_percent`] instead, since it rewrites
    /// the whole call, not just the selector).
    fn call_name(self) -> &'static str {
        match self {
            Style::Format => "format",
            Style::Sprintf => "sprintf",
            Style::Percent => unreachable!("percent style never renames a bare call"),
        }
    }
}

/// RuboCop's `AUTOCORRECTABLE_METHODS`: conversion methods known not to
/// return an `Array`, safe to keep as a positional `format`/`sprintf`
/// argument.
fn is_known_conversion_method(name: &[u8]) -> bool {
    matches!(name, b"to_d" | b"to_f" | b"to_h" | b"to_i" | b"to_r" | b"to_s" | b"to_sym")
}

/// RuboCop's (confusingly named) `autocorrectable?`, feeding
/// `variable_argument?`: `true` means the `%` RHS's value type is ambiguous
/// enough that rewriting it as a `format`/`sprintf` positional argument
/// could change behaviour -- a bare local-variable read, or a call to
/// anything but a [`is_known_conversion_method`] method. Despite the name,
/// `true` here means "not safe to autocorrect".
fn is_ambiguous_percent_argument(arg: &Node<'_>) -> bool {
    if arg.kind() == NodeKind::LocalVariableReadNode {
        return true;
    }
    arg.as_call_node().is_some_and(|call| !is_known_conversion_method(call.name().as_slice()))
}

/// A hash literal, braced (`NodeKind::HashNode`) or bare trailing keywords
/// (`NodeKind::KeywordHashNode`) -- both read as "hash" shaped upstream.
fn is_hash_like(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::HashNode | NodeKind::KeywordHashNode)
}

/// RuboCop-AST's `ASSIGNMENTS` (`EQUALS_ASSIGNMENTS` +
/// `SHORTHAND_ASSIGNMENTS`): every simple (`=`) and compound (`+=`, `||=`,
/// `&&=`) write, for every assignable target kind whitequark's single
/// `op_asgn`/`or_asgn`/`and_asgn` wrapper covers regardless of target --
/// Prism instead gives each target kind its own Operator/And/Or write node.
fn is_assignment_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::MultiWriteNode
    )
}

/// RuboCop-AST's `OPERATOR_METHODS`.
fn is_operator_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"|" | b"^"
            | b"&"
            | b"<=>"
            | b"=="
            | b"==="
            | b"=~"
            | b">"
            | b">="
            | b"<"
            | b"<="
            | b"<<"
            | b">>"
            | b"+"
            | b"-"
            | b"*"
            | b"/"
            | b"%"
            | b"**"
            | b"~"
            | b"+@"
            | b"-@"
            | b"!@"
            | b"~@"
            | b"[]"
            | b"[]="
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

/// RuboCop's `requires_parentheses?`: an argument that binds looser than
/// `%` (an assignment, a ternary/`&&`/`||`, or an un-parenthesized operator
/// call) must be wrapped in parens to keep its meaning as the RHS of `%`.
/// Upstream also lists `:range`, but under both whitequark and Prism a
/// range literal's own node type is `irange`/`erange` (inclusive/exclusive),
/// never bare `range`, so that arm is unreachable in practice and is not
/// ported.
fn requires_parentheses(arg: &Node<'_>) -> bool {
    if is_assignment_kind(arg.kind())
        || matches!(arg.kind(), NodeKind::IfNode | NodeKind::AndNode | NodeKind::OrNode)
    {
        return true;
    }
    arg.as_call_node().is_some_and(|call| {
        is_operator_method(call.name().as_slice()) && call.closing_loc().is_none()
    })
}

/// The source text of `span`.
fn source_of(ctx: &Context<'_>, span: ruby_source::Span) -> String {
    String::from_utf8_lossy(ctx.text(span)).into_owned()
}

/// Every element's own source, comma-joined -- RuboCop's
/// `node.children.map(&:source).join(', ')` for an array or hash RHS being
/// splatted back into positional/keyword arguments.
fn join_children_source(list: &NodeList<'_>, ctx: &Context<'_>) -> String {
    list.iter().map(|child| source_of(ctx, child.span())).collect::<Vec<_>>().join(", ")
}

/// RuboCop's `format_single_parameter`: the lone remaining argument when
/// converting a `format`/`sprintf` call to `%`, rendered as the operator's
/// RHS.
fn format_single_parameter(arg: &Node<'_>, ctx: &Context<'_>) -> String {
    if let Some(splat) = arg.as_splat_node() {
        // `format(fmt, *args)` is equivalent to `fmt % args`: unwrap the
        // splat and render the expression it splats.
        return match splat.expression() {
            Some(expr) => format_single_parameter(&expr, ctx),
            None => source_of(ctx, arg.span()),
        };
    }
    let source = source_of(ctx, arg.span());
    if is_hash_like(arg.kind()) {
        return format!("{{ {source} }}");
    }
    if requires_parentheses(arg) {
        return format!("({source})");
    }
    source
}

/// Enforce the use of Kernel#sprintf, Kernel#format or String#%.
#[derive(Debug, Clone)]
pub struct FormatString {
    style: Style,
}

impl FormatString {
    /// RuboCop's `formatter` node-matcher: which of the three format-string
    /// shapes (see the module doc) `call` is, if any.
    fn detect(call: &CallNode<'_>) -> Option<Style> {
        let name = call.name();
        match name.as_slice() {
            b"sprintf" | b"format" if call.receiver().is_none() => {
                if call.arguments()?.arguments().len() < 2 {
                    return None;
                }
                Some(if name.as_slice() == b"sprintf" { Style::Sprintf } else { Style::Format })
            }
            b"%" => {
                let receiver = call.receiver()?;
                let arg = call.arguments()?.arguments().first()?;
                let is_string_receiver = matches!(
                    receiver.kind(),
                    NodeKind::StringNode | NodeKind::InterpolatedStringNode
                );
                let is_collection_arg =
                    arg.kind() == NodeKind::ArrayNode || is_hash_like(arg.kind());
                (is_string_receiver || is_collection_arg).then_some(Style::Percent)
            }
            _ => None,
        }
    }

    /// RuboCop's `autocorrect_from_percent`: `receiver % arg` ->
    /// `style(receiver, args...)`. `None` when the RHS is
    /// [`is_ambiguous_percent_argument`] -- the offense is still reported,
    /// just with no fix attached.
    fn autocorrect_from_percent(&self, call: &CallNode<'_>, ctx: &Context<'_>) -> Option<Fix> {
        let receiver = call.receiver()?;
        let arg = call.arguments()?.arguments().first()?;
        if is_ambiguous_percent_argument(&arg) {
            return None;
        }
        let args = match arg.kind() {
            NodeKind::ArrayNode => join_children_source(&arg.as_array_node()?.elements(), ctx),
            NodeKind::HashNode => join_children_source(&arg.as_hash_node()?.elements(), ctx),
            _ => source_of(ctx, arg.span()),
        };
        let receiver_src = source_of(ctx, receiver.span());
        let corrected = format!("{}({receiver_src}, {args})", self.style.call_name());
        Some(Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(call.as_node().span(), corrected.into_bytes())],
        })
    }

    /// RuboCop's `autocorrect_to_percent`: `style(fmt, args...)` -> `fmt %
    /// rhs`, always safe (the format string and every argument keep their
    /// own source).
    fn autocorrect_to_percent(call: &CallNode<'_>, ctx: &Context<'_>) -> Fix {
        let args =
            call.arguments().expect("format/sprintf offense always has arguments").arguments();
        let format_arg = args.first().expect("checked >= 2 arguments in `detect`");
        let format_src = source_of(ctx, format_arg.span());
        let param_args: Vec<Node<'_>> = args.iter().skip(1).collect();
        let rhs = if param_args.len() == 1 {
            format_single_parameter(&param_args[0], ctx)
        } else {
            format!(
                "[{}]",
                param_args.iter().map(|a| source_of(ctx, a.span())).collect::<Vec<_>>().join(", ")
            )
        };
        let corrected = format!("{format_src} % {rhs}");
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(call.as_node().span(), corrected.into_bytes())],
        }
    }

    /// RuboCop's `autocorrect`, dispatching on which shape `call` is.
    fn fix(&self, call: &CallNode<'_>, ctx: &Context<'_>) -> Option<Fix> {
        match call.name().as_slice() {
            b"%" => self.autocorrect_from_percent(call, ctx),
            b"format" | b"sprintf" => Some(match self.style {
                Style::Percent => Self::autocorrect_to_percent(call, ctx),
                Style::Format | Style::Sprintf => Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(
                        call.message_loc()?.span(),
                        self.style.call_name().as_bytes().to_vec(),
                    )],
                },
            }),
            _ => None,
        }
    }
}

impl Rule for FormatString {
    const META: RuleMeta = RuleMeta {
        name: "Style/FormatString",
        department: Department::Style,
        summary: "Enforce the use of Kernel#sprintf, Kernel#format or String#%.",
        explanation: "\
Enforces the use of a single string formatting utility. Valid options
include `Kernel#format`, `Kernel#sprintf`, and `String#%`.

The detection of `String#%` cannot be implemented in a reliable manner for
all cases, so only two scenarios are considered -- if the first argument is
a string literal and if the second argument is an array literal.

Autocorrection will be applied when the argument is a literal or uses a
known built-in conversion method such as `to_d`, `to_f`, `to_h`, `to_i`,
`to_r`, `to_s`, and `to_sym` on variables, provided that their return value
is not an array. For example, when using `to_s`, `'%s' % [1, 2, 3].to_s`
can be autocorrected without any incompatibility:

```ruby
'%s' % [1, 2, 3]        #=> '1'
format('%s', [1, 2, 3]) #=> '[1, 2, 3]'
'%s' % [1, 2, 3].to_s   #=> '[1, 2, 3]'
```

With `EnforcedStyle: format` (default):

```ruby
# bad
puts sprintf('%10s', 'foo')
puts '%10s' % 'foo'

# good
puts format('%10s', 'foo')
```

With `EnforcedStyle: sprintf`:

```ruby
# bad
puts format('%10s', 'foo')
puts '%10s' % 'foo'

# good
puts sprintf('%10s', 'foo')
```

With `EnforcedStyle: percent`:

```ruby
# bad
puts format('%10s', 'foo')
puts sprintf('%10s', 'foo')

# good
puts '%10s' % 'foo'
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("format"),
            allowed: &["format", "sprintf", "percent"],
            doc: "Which formatting utility to enforce.",
        }],
        blind_spots: "\
Only three call shapes are ever considered: a receiverless `format`/
`sprintf` call with at least two arguments, `str_or_dstr % arg` for any
`arg`, and `anything_else % (array_or_hash_literal)`. A `%` call whose
receiver's type cannot be inferred from its own literal syntax (e.g. through
a local variable or method call known by other means to return a `String`)
is not flagged unless its RHS happens to be an array or hash literal,
matching RuboCop's own documented limitation.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "sprintf" => Style::Sprintf,
            "percent" => Style::Percent,
            _ => Style::Format,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        let Some(detected) = Self::detect(&call) else { return };
        if detected == self.style {
            return;
        }
        let Some(span) = call.message_loc().map(|loc| loc.span()) else { return };
        let message = format!("Favor `{}` over `{}`.", self.style.display(), detected.display());

        match self.fix(&call, ctx) {
            Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
            None => ctx.report(&Self::META, span, message),
        }
    }
}

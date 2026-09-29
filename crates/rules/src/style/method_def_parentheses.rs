//! `Style/MethodDefParentheses`, ported from RuboCop's
//! `lib/rubocop/cop/style/method_def_parentheses.rb`, plus the `parentheses?`/
//! `add_parentheses` helpers from `lib/rubocop/cop/util.rb` its autocorrection
//! uses.
//!
//! Prism unifies whitequark's `def`/`defs` (`on_def`/`on_defs`, aliased to
//! the same handler upstream) into a single `DefNode` with an optional
//! `receiver`, so one `NodeKind::DefNode` subscription covers both instance
//! and singleton method definitions; there is no safe-navigation variant to
//! worry about.
//!
//! Whitequark's `def` node always carries an `args` child, even when the
//! method has no parameters at all -- an empty `(args)` node whose own
//! source range spans the parentheses when present (`parentheses?(args)` is
//! `args.loc_is?(:end, ')')`) or is zero-width right after the method name
//! when they are absent. Prism instead makes `DefNode::parameters()` `None`
//! whenever there are zero parameters -- including `def foo()` with empty
//! parens -- and puts the parens themselves on the `DefNode` (`lparen_loc`/
//! `rparen_loc`), independent of whether `parameters()` is `Some`. So
//! `has_parens` (`def.lparen_loc().is_some()`) stands in for
//! `parentheses?(args)`, and the offense location for unwanted parens is
//! built from `lparen_loc`/`rparen_loc` directly rather than from the
//! params node, which would be `None` for `def foo()`.
//!
//! Upstream's `anonymous_arguments?` walks `node.arguments` (every
//! parameter, flattened) looking for a `forward_arg` (bare `...`), an
//! anonymous `restarg`/`kwrestarg` (`*`/`**` with no name), or checks
//! `node.last_argument` for an anonymous `blockarg` (`&`). Prism's `...` is
//! not a standalone parameter in the flattened list at all: it occupies
//! `ParametersNode::keyword_rest()` as a `ForwardingParameterNode`, the same
//! slot a real `**kwrest` would use. A block parameter is always
//! syntactically last when present, so checking `ParametersNode::block()`
//! directly reproduces the `last_argument` check without building the full
//! flattened list.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{DefNode, ParametersNode};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `MSG_PRESENT`.
const MSG_PRESENT: &str = "Use def without parentheses.";
/// RuboCop's `MSG_MISSING`.
const MSG_MISSING: &str = "Use def with parentheses when there are parameters.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Parentheses,
    NoParentheses,
    NoParenthesesExceptMultiline,
}

/// Checks if the method definitions have or don't have parentheses.
#[derive(Debug, Clone)]
pub struct MethodDefParentheses {
    style: Style,
}

impl MethodDefParentheses {
    /// RuboCop's `require_parentheses?`.
    fn require_parentheses(&self, params: Option<ParametersNode<'_>>, ctx: &Context<'_>) -> bool {
        self.style == Style::Parentheses
            || (self.style == Style::NoParenthesesExceptMultiline
                && params.is_some_and(|p| !ctx.is_single_line(p.location().span())))
    }

    /// RuboCop's `missing_parentheses`: reports `MSG_MISSING` at the params'
    /// own span (no parens) and, on autocorrect, RuboCop's `add_parentheses`
    /// `args_type?` branch -- replace the run of horizontal whitespace
    /// before the params with `(`, then insert `)` right after them.
    fn missing_parentheses(params: ParametersNode<'_>, ctx: &mut Context<'_>) {
        let span = params.location().span();
        let leading = ctx.with_surrounding_space(span, Side::Left, true, false);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG_MISSING,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(Span::new(leading.start, span.start), b"(".as_slice()),
                    Edit::insert(span.end, b")".as_slice()),
                ],
            },
        );
    }

    /// RuboCop's `unwanted_parentheses`: reports `MSG_PRESENT` over the
    /// parens (and any params between them) and, on autocorrect,
    /// `correct_arguments` -- replace `(` with a single space, remove `)`.
    fn unwanted_parentheses(def: &DefNode<'_>, ctx: &mut Context<'_>) {
        let lparen = def.lparen_loc().expect("checked by caller").span();
        let rparen = def.rparen_loc().expect("a def with lparen always has rparen").span();
        let span = Span::new(lparen.start, rparen.end);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG_PRESENT,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(lparen, b" ".as_slice()),
                    Edit::replace(rparen, b"".as_slice()),
                ],
            },
        );
    }
}

/// RuboCop's `forced_parentheses?`: regardless of style, parentheses are
/// necessary for an endless method or an argument list containing a
/// `forward-arg` (`...`), an anonymous rest/keyword-rest forwarding
/// (`*`/`**`), or an anonymous block forwarding (`&`) -- removing them would
/// be a syntax error.
fn forced_parentheses(def: &DefNode<'_>, params: Option<ParametersNode<'_>>) -> bool {
    if def.equal_loc().is_some() {
        return true;
    }
    let Some(params) = params else { return false };
    if params.rest().and_then(|r| r.as_rest_parameter_node()).is_some_and(|r| r.name().is_none()) {
        return true;
    }
    if let Some(kwrest) = params.keyword_rest() {
        if kwrest.as_forwarding_parameter_node().is_some() {
            return true;
        }
        if kwrest.as_keyword_rest_parameter_node().is_some_and(|r| r.name().is_none()) {
            return true;
        }
    }
    params.block().is_some_and(|b| b.name().is_none())
}

impl Rule for MethodDefParentheses {
    const META: RuleMeta = RuleMeta {
        name: "Style/MethodDefParentheses",
        department: Department::Style,
        summary: "Checks if the method definitions have or don't have parentheses.",
        explanation: "\
Checks for parentheses around the arguments in method definitions. Both \
instance and class/singleton methods are checked.

Regardless of style, parentheses are necessary for:

1. Endless methods
2. Argument lists containing a `forward-arg` (`...`)
3. Argument lists containing an anonymous rest arguments forwarding (`*`)
4. Argument lists containing an anonymous keyword rest arguments forwarding (`**`)
5. Argument lists containing an anonymous block forwarding (`&`)

Removing the parens would be a syntax error here.

```ruby
# EnforcedStyle: require_parentheses (default)
# The `require_parentheses` style requires method definitions
# to always use parentheses

# bad
def bar num1, num2
  num1 + num2
end

# good
def bar(num1, num2)
  num1 + num2
end

# EnforcedStyle: require_no_parentheses
# The `require_no_parentheses` style requires method definitions
# to never use parentheses

# bad
def bar(num1, num2)
  num1 + num2
end

# good
def bar num1, num2
  num1 + num2
end

# EnforcedStyle: require_no_parentheses_except_multiline
# The `require_no_parentheses_except_multiline` style prefers no
# parentheses when method definition arguments fit on single line,
# but prefers parentheses when arguments span multiple lines.

# bad
def bar(num1, num2)
  num1 + num2
end

# good
def bar num1, num2
  num1 + num2
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("require_parentheses"),
            allowed: &[
                "require_parentheses",
                "require_no_parentheses",
                "require_no_parentheses_except_multiline",
            ],
            doc: "Whether method definitions require parentheses around their arguments.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "require_no_parentheses" => Style::NoParentheses,
            "require_no_parentheses_except_multiline" => Style::NoParenthesesExceptMultiline,
            _ => Style::Parentheses,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        let params = def.parameters();
        let has_parens = def.lparen_loc().is_some();

        if self.require_parentheses(params, ctx) {
            if let Some(params) = params {
                if !has_parens {
                    Self::missing_parentheses(params, ctx);
                }
            }
        } else if forced_parentheses(&def, params) {
            // Correct style already: parens are syntactically required here
            // regardless of `EnforcedStyle`.
        } else if has_parens {
            Self::unwanted_parentheses(&def, ctx);
        }
    }
}

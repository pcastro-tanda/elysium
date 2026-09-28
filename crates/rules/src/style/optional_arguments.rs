//! `Style/OptionalArguments`, ported from RuboCop's
//! `lib/rubocop/cop/style/optional_arguments.rb`.
//!
//! Upstream walks `node.arguments` (every parameter, in declaration order:
//! required positionals, optional positionals, a splat, post-splat required
//! positionals, keywords, a double-splat, a block parameter -- whitequark's
//! single flat list, in exactly that order per Ruby's own grammar) and
//! records the index of every `optarg_type?`/`arg_type?` entry, then flags
//! an optional argument whenever its index is not greater than the largest
//! required-argument index (breaking out -- "there can only be one group of
//! optional arguments" -- the first time it is).
//!
//! Prism splits that single list into [`ruby_ast::node::ParametersNode`]'s
//! separate `requireds`/`optionals`/`rest`/`posts`/`keywords`/`keyword_rest`/
//! `block` accessors, but Ruby's grammar only ever allows *one* contiguous
//! run of optional positionals, followed optionally by post-splat required
//! positionals (`posts`) -- `def foo(a = 1, b, c = 2)` is a syntax error
//! ("unexpected parameter order"), confirmed against Prism directly. Since
//! `requireds` (if any) always precedes every entry in `optionals`, and
//! `posts` (if any) always follows every entry in `optionals`, the
//! upstream index/break logic collapses to a single question: are there any
//! `posts`? If so, every `optionals` entry is flagged (each one's index is
//! necessarily below every `posts` index, matching upstream's `break`
//! condition never triggering); if not, `requireds`' own max index is
//! necessarily below `optionals`' minimum index, so upstream's `break`
//! triggers on the very first optional argument and none are flagged.
//! Keyword parameters never carry `arg_type?`/`optarg_type?` upstream (a
//! bare `arg_type?`/`optarg_type?` check is false for `kwarg`/`kwoptarg`),
//! so they play no role in either implementation -- matching the
//! `named params` spec context, where optional arguments before required or
//! optional keyword arguments are never flagged.
//!
//! This cop covers both `def` and `defs` (`self.foo`) via a single
//! `DefNode` subscription: Prism represents a singleton method definition as
//! an ordinary `DefNode` with a populated `receiver`, unlike whitequark's
//! distinct `defs` node kind, so no separate handling is needed.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Optional arguments should appear at the end of the argument list.";

/// Checks for optional arguments to methods that do not come at the end of
/// the argument list.
///
/// # Examples
///
/// ```ruby
/// # bad
/// def foo(a = 1, b, c)
/// end
///
/// # good
/// def baz(a, b, c = 1)
/// end
///
/// def foobar(a = 1, b = 2, c = 3)
/// end
/// ```
#[derive(Debug, Clone)]
pub struct OptionalArguments;

impl Rule for OptionalArguments {
    const META: RuleMeta = RuleMeta {
        name: "Style/OptionalArguments",
        department: Department::Style,
        summary: "Checks for optional arguments to methods that do not come at the end of the \
                  argument list.",
        explanation: "\
Checks for optional arguments to methods
that do not come at the end of the argument list.

```ruby
# bad
def foo(a = 1, b, c)
end

# good
def baz(a, b, c = 1)
end

def foobar(a = 1, b = 2, c = 3)
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "\
This cop is unsafe: changing a method signature implicitly changes call-site behaviour. RuboCop
reports it regardless (there is no `safe_autocorrect` opt-out and no autocorrection is offered); this
port matches that -- it never suppresses the offense on safety grounds and, like upstream, does not
attempt to fix it.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let def = node.as_def_node().expect("kind matched");
        let Some(params) = def.parameters() else { return };

        let optionals = params.optionals();
        if optionals.is_empty() {
            return;
        }
        // RuboCop's `argument_positions`/`each_misplaced_optional_arg`: since
        // Ruby's grammar only allows one contiguous run of optional
        // positionals, a required positional can only follow it as a
        // post-splat `posts` entry -- see the module doc.
        if params.posts().is_empty() {
            return;
        }

        for argument in &optionals {
            ctx.report(&Self::META, argument.span(), MSG);
        }
    }
}

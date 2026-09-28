//! `Security/Open`, ported from RuboCop's
//! `lib/rubocop/cop/security/open.rb`.
//!
//! Upstream's `open?` node-matcher restricts to `RESTRICT_ON_SEND = %i[open]`
//! plus a receiver shape of either `nil?` (bare `open`, i.e. `Kernel#open`)
//! or a bare-or-toplevel-qualified `URI` constant (`URI.open`/`::URI.open`;
//! `Foo::URI.open` does not match). [`is_uri_receiver`] mirrors that second
//! shape the same way `Lint/UselessAccessModifier`/`Lint/DuplicateMethods`/
//! `Security/UriEscapeUnescape` do: [`ruby_ast::ext::is_bare_or_toplevel_const`]
//! for the shape, a local name check for `URI` specifically.
//!
//! # `safe?`
//!
//! Upstream's `safe?` recurses through three whitequark shapes: a plain
//! `str`, a `dstr` (recursing into its first child), and a `send` node for
//! `:+` whose *receiver* is itself a plain `str` (recursing into that
//! receiver) -- notably not into a `dstr` or another concatenation, so only
//! one level of `"literal" + expr` is ever exempted. Prism's
//! [`ruby_ast::NodeKind::StringNode`]/[`ruby_ast::NodeKind::InterpolatedStringNode`]
//! map onto `str`/`dstr` directly, and [`Open::is_safe`] reproduces the same
//! recursion, including the receiver-must-be-a-literal restriction on the
//! `+` case.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// The `URI` constant, bare or `::`-qualified only (`(const {nil? cbase}
/// :URI)`); a namespaced `Foo::URI` does not match.
fn is_uri_receiver(node: &Node<'_>) -> bool {
    if !ext::is_bare_or_toplevel_const(node) {
        return false;
    }
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"URI")
        }
        NodeKind::ConstantPathNode => node
            .as_constant_path_node()
            .and_then(|p| p.name())
            .is_some_and(|n| n.as_slice() == b"URI"),
        _ => false,
    }
}

/// RuboCop's `safe_argument?`.
fn is_safe_argument(content: &[u8]) -> bool {
    !content.is_empty() && !content.starts_with(b"|")
}

/// Checks for the use of `Kernel#open` and `URI.open` with dynamic data.
#[derive(Debug, Clone, Default)]
pub struct Open;

impl Open {
    /// RuboCop's `open?`: `node`'s receiver (`None` for bare `open`) and its
    /// first argument, if `node` is a call this cop cares about at all.
    fn matches<'pr>(call: &CallNode<'pr>) -> Option<(Option<Node<'pr>>, Node<'pr>)> {
        if call.name().as_slice() != b"open" {
            return None;
        }
        let receiver = call.receiver();
        if let Some(recv) = &receiver {
            if !is_uri_receiver(recv) {
                return None;
            }
        }
        let first_arg = call.arguments()?.arguments().first()?;
        Some((receiver, first_arg))
    }

    /// RuboCop's `safe?`.
    fn is_safe(node: &Node<'_>) -> bool {
        if let Some(s) = node.as_string_node() {
            return is_safe_argument(s.unescaped());
        }
        if let Some(interp) = node.as_interpolated_string_node() {
            return interp.parts().first().is_some_and(|first| Self::is_safe(&first));
        }
        if let Some(call) = node.as_call_node() {
            if call.name().as_slice() == b"+" {
                if let Some(receiver) = call.receiver() {
                    if receiver.as_string_node().is_some() {
                        return Self::is_safe(&receiver);
                    }
                }
            }
        }
        false
    }
}

impl Rule for Open {
    const META: RuleMeta = RuleMeta {
        name: "Security/Open",
        department: Department::Security,
        summary: "Checks for the use of `Kernel#open` and `URI.open` with dynamic data.",
        explanation: "\
`Kernel#open` and `URI.open` enable not only file access but also process
invocation by prefixing a pipe symbol (e.g., `open(\"| ls\")`).
So, it may lead to a serious security risk by using variable input to
the argument of `Kernel#open` and `URI.open`. It would be better to use
`File.open`, `IO.popen` or `URI.parse#open` explicitly.

NOTE: `open` and `URI.open` with literal strings are not flagged by this
cop.

```ruby
# bad
open(something)
open(\"| #{something}\")
open(\"| foo\")
URI.open(something)

# good
File.open(something)
IO.popen(something)
URI.parse(something).open

# good (literal strings)
open(\"foo.text\")
URI.open(\"http://example.com\")
URI.parse(url).open
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Upstream's own documented `@safety` caveat: this could register false
positives if `open` is redefined in a class and then used without a
receiver in that class. `safe?`'s `+`-concatenation branch only recurses
one level deep (the receiver must be a plain string literal, not another
concatenation or an interpolated string), matching upstream's own
`concatenated_string?` restriction exactly -- not a blind spot introduced
by this port.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let Some((receiver, first_arg)) = Self::matches(&call) else { return };
        if Self::is_safe(&first_arg) {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };
        let receiver_desc = match &receiver {
            Some(r) => format!("{}.", String::from_utf8_lossy(ctx.text(r.span()))),
            None => "Kernel#".to_string(),
        };
        let message = format!("The use of `{receiver_desc}open` is a serious security risk.");
        ctx.report(&Self::META, message_loc.span(), message);
    }
}

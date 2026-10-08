//! `Style/ObjectThen`, ported from RuboCop's
//! `lib/rubocop/cop/style/object_then.rb`.
//!
//! whitequark's `on_block`/`on_numblock`/`on_itblock` (a call wrapped in a
//! block, however its parameters are spelled) are all Prism's [`CallNode`]
//! with a [`BlockNode`] in `block()`; there is no separate "wraps a call"
//! node to dispatch on. `on_send`/`on_csend` (a bare `then`/`yield_self`
//! call passed a `&:proc` as its sole argument) is the same [`CallNode`]
//! kind with a [`BlockArgumentNode`] in `block()` instead. Both shapes are
//! handled from the single `CallNode` visitor.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "Prefer `%<prefer>s` over `%<current>s`.";

/// Enforces the use of consistent method names `Object#yield_self` or `Object#then`.
#[derive(Debug, Clone)]
pub struct ObjectThen {
    style: &'static str,
    min_ruby_version_ok: bool,
}

impl Rule for ObjectThen {
    const META: RuleMeta = RuleMeta {
        name: "Style/ObjectThen",
        department: Department::Style,
        summary:
            "Enforces the use of consistent method names `Object#yield_self` or `Object#then`.",
        explanation: "\
```ruby
# EnforcedStyle: then (default)

# bad
obj.yield_self { |x| x.do_something }

# good
obj.then { |x| x.do_something }
```

```ruby
# EnforcedStyle: yield_self

# bad
obj.then { |x| x.do_something }

# good
obj.yield_self { |x| x.do_something }
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("then"),
            allowed: &["then", "yield_self"],
            doc: "Whether to prefer `Object#then` or `Object#yield_self`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "yield_self" => "yield_self",
            _ => "then",
        };
        Ok(Self { style, min_ruby_version_ok: options.target_ruby_version() >= 2.6 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.min_ruby_version_ok {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        if name != b"then" && name != b"yield_self" {
            return;
        }

        // `node.arguments.one? && node.first_argument.block_pass_type?`: a
        // bare `obj.then(&:proc)` call (no literal block) still counts, but
        // a call with neither a literal block nor a sole `&:proc` argument
        // (e.g. a plain `obj.then`) does not.
        let has_block = call.block().is_some_and(|b| b.as_block_node().is_some());
        let has_block_pass = call.block().is_some_and(|b| b.as_block_argument_node().is_some());
        let no_regular_args = call.arguments().is_none_or(|a| a.arguments().iter().count() == 0);
        if !has_block && (!has_block_pass || !no_regular_args) {
            return;
        }

        if name == self.style.as_bytes() {
            return;
        }

        let Some(message_loc) = call.message_loc() else { return };
        let span = message_loc.span();
        let current = std::str::from_utf8(name).unwrap_or_default();
        let message = MSG.replacen("%<prefer>s", self.style, 1).replacen("%<current>s", current, 1);

        // `style == :then && node.receiver.nil? ? 'self.then' : style`
        let replacement: Vec<u8> = if self.style == "then" && call.receiver().is_none() {
            b"self.then".to_vec()
        } else {
            self.style.as_bytes().to_vec()
        };

        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
    }
}

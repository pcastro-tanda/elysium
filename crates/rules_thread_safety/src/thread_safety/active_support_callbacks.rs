//! `ThreadSafety/ActiveSupportCallbacks`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/active_support_callbacks.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// Avoid mutating `ActiveSupport` callback chains at runtime.
#[derive(Debug, Clone)]
pub struct ActiveSupportCallbacks {
    /// `requires_gem 'activesupport'`.
    supported: bool,
}

/// `constant_receiver?`: unwrap single-child parentheses, then a constant.
fn is_constant_receiver(receiver: Node<'_>) -> bool {
    let mut current = receiver;
    while let Some(parens) = current.as_parentheses_node() {
        let Some(body) = parens.body() else { return false };
        let Some(stmts) = body.as_statements_node() else { return false };
        let list = stmts.body();
        if list.len() != 1 {
            return false;
        }
        current = list.iter().next().expect("one statement");
    }
    matches!(current.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

impl Rule for ActiveSupportCallbacks {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/ActiveSupportCallbacks",
        department: Department::ThreadSafety,
        summary: "Avoid mutating ActiveSupport callback chains at runtime.",
        explanation: "\
Avoid mutating ActiveSupport callback chains at runtime.

Calls such as `User.skip_callback` and `User.set_callback` mutate callback
chains at process scope.

```ruby
# bad
Site.skip_callback(:commit, :after, :after_owner_change)

# good
class User < ApplicationRecord
  skip_callback :commit, :after, :after_owner_change
end
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.requires_gem("activesupport", &[]) })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        if name != b"set_callback" && name != b"skip_callback" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !is_constant_receiver(receiver) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let Some(dot) = call.call_operator_loc() else { return };
        let expression = format!(
            "{}{}{}",
            String::from_utf8_lossy(ctx.text(receiver.span())),
            String::from_utf8_lossy(ctx.text(dot.span())),
            String::from_utf8_lossy(name),
        );
        ctx.report(
            &Self::META,
            selector.span(),
            format!("Avoid process-wide ActiveSupport callback mutation with `{expression}`."),
        );
    }
}

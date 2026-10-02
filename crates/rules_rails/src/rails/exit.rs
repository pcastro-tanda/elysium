//! `Rails/Exit`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/exit.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const EXPLICIT_RECEIVERS: [&[u8]; 2] = [b"Kernel", b"Process"];

/// Favor `fail`, `break`, `return`, etc. over `exit` in application or library code outside of Rake files to avoid exits during unit testing or running in production.
#[derive(Debug, Clone)]
pub struct Exit;

impl Rule for Exit {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Exit",
        department: Department::Rails,
        summary: "Favor `fail`, `break`, `return`, etc. over `exit` in application or library code outside of Rake files to avoid exits during unit testing or running in production.",
        explanation: "Enforces that `exit` and `abort` calls are not used within a rails app. \
                      Valid options are instead to raise an error, break, return, or some \
                      other form of stopping execution of current request.\n\nThere are two \
                      obvious cases where `exit` is particularly harmful:\n\n* Usage in \
                      library code for your application. Even though Rails will rescue from \
                      a `SystemExit` and continue on, unit testing that library code will \
                      result in specs exiting (potentially silently if `exit(0)` is used.)\n\
                      * Usage in application code outside of the web process could result in \
                      the program exiting, which could result in the code failing to run and \
                      do its job.\n\n```ruby\n# bad\nexit(0)\n\n# good\nraise 'a bad error \
                      has happened'\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        if !matches!(name, b"exit" | b"exit!" | b"abort") {
            return;
        }
        // More than one argument likely means it is a different `exit`
        // implementation than the one we are preventing (a `&block` counts
        // as an argument in the parser's tree).
        let count = call.arguments().map_or(0, |args| args.arguments().iter().count())
            + usize::from(call.block().is_some_and(|b| b.as_block_argument_node().is_some()));
        if count > 1 {
            return;
        }
        if let Some(receiver) = call.receiver() {
            // `_a, receiver_node_class, _c = *receiver_node`: the second
            // child, which is the constant's or the call's name.
            let second = match receiver.kind() {
                NodeKind::ConstantReadNode => {
                    receiver.as_constant_read_node().map(|c| c.name().as_slice().to_vec())
                }
                NodeKind::ConstantPathNode => receiver
                    .as_constant_path_node()
                    .and_then(|c| c.name().map(|n| n.as_slice().to_vec())),
                NodeKind::CallNode => receiver.as_call_node().map(|c| c.name().as_slice().to_vec()),
                _ => None,
            };
            if !second.is_some_and(|n| EXPLICIT_RECEIVERS.contains(&n.as_slice())) {
                return;
            }
        }
        let Some(selector) = call.message_loc() else { return };
        let message =
            format!("Do not use `{}` in Rails applications.", String::from_utf8_lossy(name));
        ctx.report(&Self::META, selector.span(), message);
    }
}

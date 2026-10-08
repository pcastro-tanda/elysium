//! `Rails/DynamicFindBy`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/dynamic_find_by.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// Use `find_by` instead of dynamic `find_by_*`.
#[derive(Debug, Clone)]
pub struct DynamicFindBy {
    allowed_methods: Vec<String>,
    allowed_receivers: Vec<String>,
    whitelist: Vec<String>,
    /// Number of enclosing classes whose superclass is `ApplicationRecord`
    /// or `ActiveRecord::Base` (`inherit_active_record_base?`).
    active_record_depth: usize,
}

impl Rule for DynamicFindBy {
    const META: RuleMeta = RuleMeta {
        name: "Rails/DynamicFindBy",
        department: Department::Rails,
        summary: "Use `find_by` instead of dynamic `find_by_*`.",
        explanation: "Checks dynamic `find_by_*` methods. Use `find_by` instead of dynamic \
                      method.\n\nIt is certainly unsafe when not configured properly, i.e. \
                      user-defined `find_by_xxx` method is not added to cop's \
                      `AllowedMethods`.\n\n```ruby\n# bad\nUser.find_by_name(name)\n\
                      User.find_by_name_and_email(name)\nUser.find_by_email!(name)\n\n# good\n\
                      User.find_by(name: name)\nUser.find_by(name: name, email: email)\n\
                      User.find_by!(email: email)\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::ClassNode],
        config: &[
            ConfigOption {
                name: "Whitelist",
                default: ConfigDefault::StrList(&["find_by_sql", "find_by_token_for"]),
                allowed: &[],
                doc: "Deprecated, use `AllowedMethods` instead.",
            },
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&["find_by_sql", "find_by_token_for"]),
                allowed: &[],
                doc: "Dynamic finder methods that are allowed.",
            },
            ConfigOption {
                name: "AllowedReceivers",
                default: ConfigDefault::StrList(&["Gem::Specification", "page"]),
                allowed: &[],
                doc: "Receivers (by source) whose dynamic finders are allowed.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allowed_methods: options.str_list("AllowedMethods"),
            allowed_receivers: options.str_list("AllowedReceivers"),
            whitelist: options.str_list("Whitelist"),
            active_record_depth: 0,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(class) = node.as_class_node() {
            if class.superclass().is_some_and(|s| is_active_record(&s)) {
                self.active_record_depth += 1;
            }
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let receiver = call.receiver();
        if (receiver.is_none() && self.active_record_depth == 0)
            || self.allowed_invocation(&call, receiver.as_ref(), ctx)
        {
            return;
        }

        let method_name = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        let Some((columns, bang)) = parse_method(&method_name) else { return };
        let static_name = if bang { "find_by!" } else { "find_by" };

        // Arguments as whitequark sees them: a block-pass is one too.
        let mut arguments: Vec<Node<'_>> = Vec::new();
        if let Some(args) = call.arguments() {
            arguments.extend(args.arguments().iter());
        }
        if let Some(block) = call.block() {
            if block.as_block_argument_node().is_some() {
                arguments.push(block);
            }
        }
        if columns.len() != arguments.len()
            || arguments.iter().any(|a| {
                matches!(
                    a.kind(),
                    NodeKind::HashNode | NodeKind::KeywordHashNode | NodeKind::SplatNode
                )
            })
        {
            return;
        }

        let Some(selector) = call.message_loc() else { return };
        let mut edits = vec![Edit::replace(selector.span(), static_name.as_bytes().to_vec())];
        for (keyword, argument) in columns.iter().zip(&arguments) {
            edits.push(Edit::insert(argument.span().start, format!("{keyword}: ").into_bytes()));
        }
        let message = format!("Use `{static_name}` instead of dynamic `{method_name}`.");
        ctx.report_with_fix(
            &Self::META,
            call_span_excluding_block(&call),
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if let Some(class) = node.as_class_node() {
            if class.superclass().is_some_and(|s| is_active_record(&s)) {
                self.active_record_depth -= 1;
            }
        }
    }
}

impl DynamicFindBy {
    fn allowed_invocation(
        &self,
        call: &ruby_ast::node::CallNode<'_>,
        receiver: Option<&Node<'_>>,
        ctx: &Context<'_>,
    ) -> bool {
        let name = String::from_utf8_lossy(call.name().as_slice());
        if self.allowed_methods.iter().any(|m| *m == name) {
            return true;
        }
        if let Some(receiver) = receiver {
            let source = String::from_utf8_lossy(ctx.text(receiver.span()));
            if self.allowed_receivers.iter().any(|r| *r == source) {
                return true;
            }
        }
        self.whitelist.iter().any(|m| *m == name)
    }
}

/// `METHOD_PATTERN = /^find_by_(.+?)(!)?$/` plus `column_keywords`: the
/// column names (`split('_and_')`, trailing empties dropped) and whether the
/// name ends in `!`.
fn parse_method(name: &str) -> Option<(Vec<&str>, bool)> {
    let rest = name.strip_prefix("find_by_")?;
    if rest.is_empty() {
        return None;
    }
    let (columns, bang) = match rest.strip_suffix('!') {
        Some(stripped) if !stripped.is_empty() => (stripped, true),
        _ => (rest, false),
    };
    let mut keywords: Vec<&str> = columns.split("_and_").collect();
    while keywords.last().is_some_and(|k| k.is_empty()) {
        keywords.pop();
    }
    Some((keywords, bang))
}

/// `active_record?`: `ApplicationRecord` or `ActiveRecord::Base`, optionally
/// rooted at `::`.
fn is_active_record(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
        && matches!(const_name(node).as_deref(), Some("ApplicationRecord" | "ActiveRecord::Base"))
}

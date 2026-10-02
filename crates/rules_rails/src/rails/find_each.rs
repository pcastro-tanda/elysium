//! `Rails/FindEach`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/find_each.rb` (with `ActiveRecordHelper`,
//! `AllowedMethods` and `AllowedPattern`).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::ext::const_name;
use ruby_ast::node::CallNode;
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `find_each` instead of `each`.";

const SCOPE_METHODS: &[&[u8]] = &[
    b"all",
    b"eager_load",
    b"includes",
    b"joins",
    b"left_joins",
    b"left_outer_joins",
    b"not",
    b"or",
    b"preload",
    b"references",
    b"unscoped",
    b"where",
];

/// Prefer `all.find_each` over `all.each`.
#[derive(Debug, Clone)]
pub struct FindEach {
    allowed_methods: Vec<String>,
    allowed_patterns: Vec<Regex>,
    /// Number of enclosing classes whose superclass is `ApplicationRecord`
    /// or `ActiveRecord::Base` (`inherit_active_record_base?`).
    active_record_depth: usize,
}

impl Rule for FindEach {
    const META: RuleMeta = RuleMeta {
        name: "Rails/FindEach",
        department: Department::Rails,
        summary: "Prefer all.find_each over all.each.",
        explanation: "Identifies usages of `all.each` and change them to use `all.find_each` \
                      instead.\n\nThis cop is unsafe if the receiver object is not an Active \
                      Record object. Also, `all.each` returns an `Array` instance and \
                      `all.find_each` returns nil, so the return values are different.\n\n\
                      ```ruby\n# bad\nUser.all.each\n\n# good\nUser.all.find_each\n```\n\nWith \
                      `AllowedMethods: ['order']` or `AllowedPatterns: ['order']`, \
                      `User.order(:foo).each` is left alone.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode, NodeKind::ClassNode],
        config: &[
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&["order", "limit", "select", "lock"]),
                allowed: &[],
                doc: "Methods that don't work well with `find_each`.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Regular expressions of method names that don't work well with `find_each`.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mut allowed_methods = options.str_list("AllowedMethods");
        allowed_methods.extend(options.str_list("IgnoredMethods"));
        allowed_methods.extend(options.str_list("ExcludedMethods"));
        let mut patterns = options.str_list("AllowedPatterns");
        patterns.extend(options.str_list("IgnoredPatterns"));
        let allowed_patterns = patterns.iter().filter_map(|p| Regex::new(p).ok()).collect();
        Ok(Self { allowed_methods, allowed_patterns, active_record_depth: 0 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(class) = node.as_class_node() {
            if class.superclass().is_some_and(|s| is_active_record(&s)) {
                self.active_record_depth += 1;
            }
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"each" || call.is_safe_navigation() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        // `node.receiver&.send_type?`: a plain `send` (no safe navigation, no
        // literal block of its own).
        let Some(receiver_call) = receiver.as_call_node() else { return };
        if receiver_call.is_safe_navigation()
            || receiver_call.block().is_some_and(|b| b.as_block_node().is_some())
            || !SCOPE_METHODS.contains(&receiver_call.name().as_slice())
        {
            return;
        }
        if receiver_call.receiver().is_none() && self.active_record_depth == 0 {
            return;
        }
        if self.ignored(&call, &receiver_call) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let span = selector.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, b"find_each".to_vec())],
            },
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

impl FindEach {
    fn name_allowed(&self, name: &[u8]) -> bool {
        let name = String::from_utf8_lossy(name);
        self.allowed_methods.iter().any(|m| *m == name)
            || self.allowed_patterns.iter().any(|p| p.is_match(&name))
    }

    /// `ignored?`.
    fn ignored(&self, call: &CallNode<'_>, receiver: &CallNode<'_>) -> bool {
        // `active_model_error_where?`
        if receiver.name().as_slice() == b"where"
            && receiver
                .receiver()
                .and_then(|r| r.as_call_node())
                .is_some_and(|r| {
                    !r.is_safe_navigation()
                        && r.name().as_slice() == b"errors"
                        && !r.block().is_some_and(|b| b.as_block_node().is_some())
                })
        {
            return true;
        }
        // `node.each_node(:send).map(&:method_name)`: the call itself and its
        // descendants, without the call's own literal block (whitequark wraps
        // the `send` in a `block` node instead).
        let mut found = self.name_allowed(call.name().as_slice());
        let mut check = |node: &Node<'_>| {
            if let Some(inner) = node.as_call_node() {
                if !inner.is_safe_navigation() && self.name_allowed(inner.name().as_slice()) {
                    found = true;
                }
            }
        };
        let mut visit = |node: &Node<'_>| {
            check(node);
            each_descendant(node, &mut check);
        };
        if let Some(receiver) = call.receiver() {
            visit(&receiver);
        }
        if let Some(arguments) = call.arguments() {
            for argument in &arguments.arguments() {
                visit(&argument);
            }
        }
        if let Some(block) = call.block() {
            if block.as_block_argument_node().is_some() {
                visit(&block);
            }
        }
        found
    }
}

/// `active_record?`: `ApplicationRecord` or `ActiveRecord::Base`, optionally
/// rooted at `::`.
fn is_active_record(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
        && matches!(
            const_name(node).as_deref(),
            Some("ApplicationRecord" | "ActiveRecord::Base")
        )
}

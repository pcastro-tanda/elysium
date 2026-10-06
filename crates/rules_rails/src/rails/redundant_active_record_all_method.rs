//! `Rails/RedundantActiveRecordAllMethod`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/redundant_active_record_all_method.rb` (with the
//! `ActiveRecordHelper#inherit_active_record_base?` and core
//! `AllowedReceivers` mixins).
//!
//! RuboCop visits the `all` send and looks at its parent; Prism hands us the
//! outer call, so this port subscribes to the query-method call and reads
//! the `all` call off its receiver.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use rules::style::collection_compact::receiver_name;

const MSG: &str = "Redundant `all` detected.";

/// Defined methods in `ActiveRecord::Querying::QUERYING_METHODS` on
/// activerecord 7.1.0.
const QUERYING_METHODS: &[&str] = &[
    "and",
    "annotate",
    "any?",
    "async_average",
    "async_count",
    "async_ids",
    "async_maximum",
    "async_minimum",
    "async_pick",
    "async_pluck",
    "async_sum",
    "average",
    "calculate",
    "count",
    "create_or_find_by",
    "create_or_find_by!",
    "create_with",
    "delete_all",
    "delete_by",
    "destroy_all",
    "destroy_by",
    "distinct",
    "eager_load",
    "except",
    "excluding",
    "exists?",
    "extending",
    "extract_associated",
    "fifth",
    "fifth!",
    "find",
    "find_by",
    "find_by!",
    "find_each",
    "find_in_batches",
    "find_or_create_by",
    "find_or_create_by!",
    "find_or_initialize_by",
    "find_sole_by",
    "first",
    "first!",
    "first_or_create",
    "first_or_create!",
    "first_or_initialize",
    "forty_two",
    "forty_two!",
    "fourth",
    "fourth!",
    "from",
    "group",
    "having",
    "ids",
    "in_batches",
    "in_order_of",
    "includes",
    "invert_where",
    "joins",
    "last",
    "last!",
    "left_joins",
    "left_outer_joins",
    "limit",
    "lock",
    "many?",
    "maximum",
    "merge",
    "minimum",
    "none",
    "none?",
    "offset",
    "one?",
    "only",
    "optimizer_hints",
    "or",
    "order",
    "pick",
    "pluck",
    "preload",
    "readonly",
    "references",
    "regroup",
    "reorder",
    "reselect",
    "rewhere",
    "second",
    "second!",
    "second_to_last",
    "second_to_last!",
    "select",
    "sole",
    "strict_loading",
    "sum",
    "take",
    "take!",
    "third",
    "third!",
    "third_to_last",
    "third_to_last!",
    "touch_all",
    "unscope",
    "update_all",
    "where",
    "with",
    "without",
];

const POSSIBLE_ENUMERABLE_BLOCK_METHODS: &[&[u8]] =
    &[b"any?", b"count", b"find", b"none?", b"one?", b"select", b"sum"];
const SENSITIVE_METHODS_ON_ASSOCIATION: &[&[u8]] = &[b"delete_all", b"destroy_all"];

/// Detect redundant `all` used as a receiver for Active Record query methods.
#[derive(Debug, Clone)]
pub struct RedundantActiveRecordAllMethod {
    allowed_receivers: Vec<String>,
    /// Number of enclosing classes inheriting `ActiveRecord::Base` or
    /// `ApplicationRecord`.
    active_record_depth: usize,
}

impl Rule for RedundantActiveRecordAllMethod {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RedundantActiveRecordAllMethod",
        department: Department::Rails,
        summary: "Detect redundant `all` used as a receiver for Active Record query methods.",
        explanation: "Detect redundant `all` used as a receiver for Active Record query \
                      methods.\n\nFor the methods `delete_all` and `destroy_all`, this cop will \
                      only check cases where the receiver is a model. It will ignore cases where \
                      the receiver is an association (e.g., `user.articles.all.delete_all`). \
                      This is because omitting `all` from an association changes the methods \
                      from `ActiveRecord::Relation` to \
                      `ActiveRecord::Associations::CollectionProxy`, which can affect their \
                      behavior.\n\nThis cop is unsafe because false positives will occur if the \
                      receiver is not an Active Record object.\n\n```ruby\n# bad\n\
                      User.all.find(id)\nUser.all.order(:created_at)\nusers.all.where(id: ids)\n\
                      user.articles.all.order(:created_at)\n\n# good\nUser.find(id)\n\
                      User.order(:created_at)\nusers.where(id: ids)\n\
                      user.articles.order(:created_at)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::ClassNode],
        config: &[ConfigOption {
            name: "AllowedReceivers",
            default: ConfigDefault::StrList(&["ActionMailer::Preview", "ActiveSupport::TimeZone"]),
            allowed: &[],
            doc: "Receiver names (`AllowedReceivers#receiver_name`) whose `all` is never \
                  flagged.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allowed_receivers: options.str_list("AllowedReceivers"), active_record_depth: 0 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(class) = node.as_class_node() {
            if class.superclass().is_some_and(|superclass| is_active_record(&superclass)) {
                self.active_record_depth += 1;
            }
            return;
        }
        self.check(node, ctx);
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if let Some(class) = node.as_class_node() {
            if class.superclass().is_some_and(|superclass| is_active_record(&superclass)) {
                self.active_record_depth -= 1;
            }
        }
    }
}

impl RedundantActiveRecordAllMethod {
    fn check(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        // `followed_by_query_method?(node.parent)`:
        // `(send (send _ :all) QUERYING_METHODS ...)`.
        let Some(parent) = node.as_call_node() else { return };
        if parent.is_safe_navigation()
            || !QUERYING_METHODS.iter().any(|m| m.as_bytes() == parent.name().as_slice())
        {
            return;
        }
        let Some(all_node) = parent.receiver() else { return };
        let Some(all) = all_node.as_call_node() else { return };
        if all.name().as_slice() != b"all"
            || all.is_safe_navigation()
            || all.arguments().is_some()
            || all.block().is_some()
        {
            return;
        }
        if possible_enumerable_block_method(&parent) {
            return;
        }
        let receiver = all.receiver();
        // `sensitive_association_method?`.
        if receiver.as_ref().is_none_or(|r| {
            !matches!(r.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
        }) && SENSITIVE_METHODS_ON_ASSOCIATION.contains(&parent.name().as_slice())
        {
            return;
        }
        match &receiver {
            Some(receiver) => {
                if self.allowed_receivers.contains(&receiver_name(receiver, ctx)) {
                    return;
                }
            }
            None => {
                if self.active_record_depth == 0 {
                    return;
                }
            }
        }

        let Some(selector) = all.message_loc() else { return };
        let range = Span::new(selector.span().start, all_node.span().end);
        let Some(dot) = parent.call_operator_loc() else { return };
        ctx.report_with_fix(
            &Self::META,
            range,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::delete(range), Edit::delete(dot.span())],
            },
        );
    }
}

/// `possible_enumerable_block_method?`.
fn possible_enumerable_block_method(parent: &ruby_ast::node::CallNode<'_>) -> bool {
    if !POSSIBLE_ENUMERABLE_BLOCK_METHODS.contains(&parent.name().as_slice()) {
        return false;
    }
    match parent.block() {
        Some(block) if block.as_block_node().is_some() => true,
        // `first_argument&.block_pass_type?`: a lone `&block`.
        Some(block) => block.as_block_argument_node().is_some() && parent.arguments().is_none(),
        None => false,
    }
}

/// `ActiveRecordHelper#active_record?`:
/// `{(const {nil? cbase} :ApplicationRecord)
///   (const (const {nil? cbase} :ActiveRecord) :Base)}`.
fn is_active_record(node: &Node<'_>) -> bool {
    use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
    if is_bare_or_toplevel_const(node) && const_name(node).as_deref() == Some("ApplicationRecord") {
        return true;
    }
    let Some(path) = node.as_constant_path_node() else { return false };
    path.name().is_some_and(|name| name.as_slice() == b"Base")
        && path.parent().is_some_and(|parent| {
            is_bare_or_toplevel_const(&parent)
                && const_name(&parent).as_deref() == Some("ActiveRecord")
        })
}

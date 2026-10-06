//! `Rails/CompactBlank`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/compact_blank.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::BlockNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `minimum_target_rails_version 6.1`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 6.1;

/// Checks if collection can be blank-compacted with `compact_blank`.
#[derive(Debug, Clone)]
pub struct CompactBlank {
    /// `minimum_target_rails_version 6.1` holds.
    supported: bool,
    /// `target_ruby_version >= 2.6`: `filter` exists.
    has_filter: bool,
}

impl Rule for CompactBlank {
    const META: RuleMeta = RuleMeta {
        name: "Rails/CompactBlank",
        department: Department::Rails,
        summary: "Checks if collection can be blank-compacted with `compact_blank`.",
        explanation: "Checks if collection can be blank-compacted with `compact_blank`.\n\nIt \
                      is unsafe by default because false positives may occur in the blank check \
                      of block arguments to the receiver object.\n\n```ruby\n# bad\n\
                      collection.reject(&:blank?)\ncollection.reject { |_k, v| \
                      v.blank? }\ncollection.select(&:present?)\ncollection.select { |_k, v| \
                      v.present? }\n\n# good\ncollection.compact_blank\n\n# bad\n\
                      collection.delete_if(&:blank?)\ncollection.keep_if(&:present?)\n\n# \
                      good\ncollection.compact_blank!\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION,
            has_filter: options.target_ruby_version() >= 2.6,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        let name = call.name();
        let name = name.as_slice();
        let (predicate, destructive): (&[u8], bool) = match name {
            b"reject" => (b"blank?", false),
            b"delete_if" => (b"blank?", true),
            b"select" => (b"present?", false),
            b"filter" if self.has_filter => (b"present?", false),
            b"keep_if" => (b"present?", true),
            _ => return,
        };
        if call.arguments().is_some() {
            return;
        }
        let Some(block) = call.block() else { return };
        let bad = if let Some(pass) = block.as_block_argument_node() {
            pass.expression()
                .and_then(|e| e.as_symbol_node().map(|s| s.unescaped() == predicate))
                .unwrap_or(false)
        } else if let Some(block) = block.as_block_node() {
            block_matches(&block, predicate, ctx)
        } else {
            false
        };
        if !bad {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let range = Span::new(selector.span().start, node.span().end);
        let preferred = if destructive { "compact_blank!" } else { "compact_blank" };
        ctx.report_with_fix(
            &Self::META,
            range,
            format!("Use `{preferred}` instead."),
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, preferred.as_bytes().to_vec())],
            },
        );
    }
}

/// `reject_with_block?` / `select_with_block?` block part: a single block
/// argument (or the hash value of two) tested with `blank?`/`present?`.
fn block_matches(block: &BlockNode<'_>, predicate: &[u8], ctx: &Context<'_>) -> bool {
    let Some(params) = block.parameters() else { return false };
    let Some(params) = params.as_block_parameters_node() else { return false };
    let Some(body) = block.body() else { return false };
    let Some(statements) = body.as_statements_node() else { return false };
    let mut stmts = statements.body().iter();
    let (Some(only), None) = (stmts.next(), stmts.next()) else { return false };
    let Some(inner) = only.as_call_node() else { return false };
    if inner.is_safe_navigation()
        || inner.name().as_slice() != predicate
        || inner.arguments().is_some()
        || inner.block().is_some()
    {
        return false;
    }
    let Some(receiver) = inner.receiver() else { return false };
    if receiver.as_local_variable_read_node().is_none() {
        return false;
    }
    let receiver_src = ctx.text(receiver.span());

    let mut args: Vec<Span> = Vec::new();
    if let Some(p) = params.parameters() {
        args.extend(p.requireds().iter().map(|n| n.span()));
        args.extend(p.optionals().iter().map(|n| n.span()));
        args.extend(p.rest().map(|n| n.span()));
        args.extend(p.posts().iter().map(|n| n.span()));
        args.extend(p.keywords().iter().map(|n| n.span()));
        args.extend(p.keyword_rest().map(|n| n.span()));
        args.extend(p.block().map(|n| n.location().span()));
    }
    args.extend(params.locals().iter().map(|n| n.span()));
    args.sort_by_key(|s| s.start);
    match args.as_slice() {
        [only] => ctx.text(*only) == receiver_src,
        [_, second] => ctx.text(*second) == receiver_src,
        _ => false,
    }
}

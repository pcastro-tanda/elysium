//! `Rails/TransactionExitStatement`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/transaction_exit_statement.rb`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{for_each_child, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const BUILT_IN_TRANSACTION_METHODS: [&[u8]; 2] = [b"transaction", b"with_lock"];

/// Avoid the usage of `return`, `break` and `throw` in transaction blocks.
#[derive(Debug, Clone)]
pub struct TransactionExitStatement {
    transaction_methods: Vec<String>,
    reported: Vec<Span>,
    supported: bool,
}

impl TransactionExitStatement {
    fn transaction_method_name(&self, name: &[u8]) -> bool {
        BUILT_IN_TRANSACTION_METHODS.contains(&name)
            || self.transaction_methods.iter().any(|m| m.as_bytes() == name)
    }

    fn collect(&self, node: &Node<'_>, stack: &mut Vec<bool>, out: &mut Vec<(Span, &'static str)>) {
        if node.as_return_node().is_some() {
            out.push((node.span(), "return"));
        } else if node.as_break_node().is_some() {
            // `nested_block?`: the nearest enclosing block must be a transaction.
            if stack.last().copied().unwrap_or(true) {
                out.push((node.span(), "break"));
            }
        }
        let call = node.as_call_node();
        if let Some(call) = &call {
            if call.receiver().is_none() && call.name().as_slice() == b"throw" {
                out.push((node.span(), "throw"));
            }
        }
        let is_lambda = node.as_lambda_node().is_some();
        let call_is_tx =
            call.as_ref().is_some_and(|c| self.transaction_method_name(c.name().as_slice()));
        for_each_child(node, |child| {
            if is_lambda {
                stack.push(false);
            } else if call.is_some() && child.as_block_node().is_some() {
                stack.push(call_is_tx);
            } else {
                self.collect(child, stack, out);
                return;
            }
            self.collect(child, stack, out);
            stack.pop();
        });
    }
}

impl Rule for TransactionExitStatement {
    const META: RuleMeta = RuleMeta {
        name: "Rails/TransactionExitStatement",
        department: Department::Rails,
        summary: "Avoid the usage of `return`, `break` and `throw` in transaction blocks.",
        explanation: "Checks for the use of exit statements (namely `return`, `break` and \
                      `throw`) in transactions. This is due to the eventual unexpected behavior \
                      when using ActiveRecord >= 7, where transactions exited using these \
                      statements are being rollbacked rather than committed (pre ActiveRecord 7 \
                      behavior).\n\nAs alternatives, it would be more intuitive to explicitly \
                      raise an error when rollback is desired, and to use `next` when commit is \
                      desired.\n\nIf you are defining custom transaction methods, you can \
                      configure it with `TransactionMethods`.\n\nThis cop is disabled on Rails \
                      >= 7.2 because transactions were restored to their historical behavior.\n\n\
                      ```ruby\n# bad\nApplicationRecord.transaction do\n  return if \
                      user.active?\nend\n\n# good\nApplicationRecord.transaction do\n  # \
                      Rollback\n  raise \"User is active\" if user.active?\nend\n\n# good\n\
                      ApplicationRecord.transaction do\n  # Commit\n  next if user.active?\n\
                      end\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "TransactionMethods",
            default: ConfigDefault::StrList(&[]),
            allowed: &[],
            doc: "Custom methods that open a transaction.",
        }],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            transaction_methods: options.str_list("TransactionMethods"),
            reported: Vec::new(),
            supported: options.target_rails_version() < 7.2,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || !self.transaction_method_name(call.name().as_slice()) {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let Some(body) = block.body() else { return };
        // `node.right_siblings.none?(&:loop_keyword?)`: a lone loop as the body.
        if let Some(stmts) = body.as_statements_node() {
            let mut it = stmts.body().iter();
            if let (Some(only), None) = (it.next(), it.next()) {
                if only.as_while_node().is_some()
                    || only.as_until_node().is_some()
                    || only.as_for_node().is_some()
                {
                    return;
                }
            }
        }
        let mut found = Vec::new();
        self.collect(&body, &mut vec![true], &mut found);
        for (span, statement) in found {
            if self.reported.contains(&span) {
                continue;
            }
            self.reported.push(span);
            ctx.report(
                &Self::META,
                span,
                format!(
                    "Exit statement `{statement}` is not allowed. Use `raise` (rollback) or `next` (commit)."
                ),
            );
        }
    }
}

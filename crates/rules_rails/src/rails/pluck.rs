//! `Rails/Pluck`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/pluck.rb`.

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `minimum_target_rails_version 5.0`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.0;

/// Prefer `pluck` over `map { ... }`.
#[derive(Debug, Clone)]
pub struct Pluck {
    supported: bool,
    /// Block start offset -> whether the block's call has a receiver.
    block_receivers: HashMap<u32, bool>,
    /// For each enclosing block/lambda: does its call have a receiver?
    stack: Vec<bool>,
}

impl Rule for Pluck {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Pluck",
        department: Department::Rails,
        summary: "Prefer `pluck` over `map { ... }`.",
        explanation: "Enforces the use of `pluck` over `map`.\n\n`pluck` can be used instead of \
                      `map` to extract a single key from each element in an enumerable. When \
                      called on an Active Record relation, it results in a more efficient query \
                      that only selects the necessary key.\n\nThe cop ignores `map`/`collect` \
                      inside a block whose call has a receiver, since `pluck` there may cause \
                      N+1 queries.\n\nThis cop is unsafe because the model can use column \
                      aliases.\n\n```ruby\n# bad\nPost.published.map { |post| post[:title] }\n\
                      [{ a: :b, c: :d }].collect { |el| el[:a] }\n\n# good\n\
                      Post.published.pluck(:title)\n[{ a: :b, c: :d }].pluck(:a)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode, NodeKind::BlockNode, NodeKind::LambdaNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION,
            block_receivers: HashMap::new(),
            stack: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        if node.as_lambda_node().is_some() {
            self.stack.push(false);
            return;
        }
        if node.as_block_node().is_some() {
            let has_receiver = self.block_receivers.remove(&node.span().start).unwrap_or(false);
            self.stack.push(has_receiver);
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let Some(block_node) = call.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };
        self.block_receivers.insert(block_node.span().start, call.receiver().is_some());

        // `node.each_ancestor(:any_block).first&.receiver`
        if self.stack.last().copied().unwrap_or(false) {
            return;
        }
        let name = call.name();
        if name.as_slice() != b"map" && name.as_slice() != b"collect" {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };

        let Some((block_argument, key)) = Self::candidate(&block, ctx) else { return };
        let key_source = ctx.text(key.span());
        let key_is_regexp =
            key.as_regular_expression_node().is_some() || key.as_interpolated_regular_expression_node().is_some();
        if key_is_regexp || block_argument == key_source {
            return;
        }
        let mut uses_argument = false;
        each_descendant(&key, &mut |child| {
            if (child.as_local_variable_read_node().is_some()
                || child.as_it_local_variable_read_node().is_some())
                && ctx.text(child.span()) == block_argument.as_slice()
            {
                uses_argument = true;
            }
        });
        if uses_argument {
            return;
        }

        let range = Span::new(message_loc.span().start, block.closing_loc().span().end);
        let replacement = format!("pluck({})", String::from_utf8_lossy(key_source));
        let message = format!(
            "Prefer `{replacement}` over `{}`.",
            String::from_utf8_lossy(ctx.text(range))
        );
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, replacement.into_bytes())],
            },
        );
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if self.supported && (node.as_block_node().is_some() || node.as_lambda_node().is_some()) {
            self.stack.pop();
        }
    }
}

impl Pluck {
    /// `(any_block (call _ {:map :collect}) $_argument (send lvar :[] $_key))`
    /// plus `use_one_block_argument?`; returns the block argument's source and
    /// the key node.
    fn candidate<'pr>(
        block: &ruby_ast::node::BlockNode<'pr>,
        ctx: &Context<'_>,
    ) -> Option<(Vec<u8>, Node<'pr>)> {
        let params = block.parameters()?;
        let argument: Vec<u8> = if let Some(numbered) = params.as_numbered_parameters_node() {
            if numbered.maximum() != 1 {
                return None;
            }
            b"_1".to_vec()
        } else if params.as_it_parameters_node().is_some() {
            b"it".to_vec()
        } else {
            let block_params = params.as_block_parameters_node()?;
            let mut spans: Vec<Span> = Vec::new();
            if let Some(p) = block_params.parameters() {
                spans.extend(p.requireds().iter().map(|n| n.span()));
                spans.extend(p.optionals().iter().map(|n| n.span()));
                if let Some(rest) = p.rest() {
                    if rest.as_implicit_rest_node().is_none() {
                        spans.push(rest.span());
                    }
                }
                spans.extend(p.posts().iter().map(|n| n.span()));
                spans.extend(p.keywords().iter().map(|n| n.span()));
                if let Some(kwrest) = p.keyword_rest() {
                    spans.push(kwrest.span());
                }
                if let Some(blk) = p.block() {
                    spans.push(blk.as_node().span());
                }
            }
            spans.extend(block_params.locals().iter().map(|n| n.span()));
            if spans.len() != 1 {
                return None;
            }
            ctx.text(spans[0]).to_vec()
        };

        let body = block.body()?;
        let statements = body.as_statements_node()?;
        let list = statements.body();
        if list.len() != 1 {
            return None;
        }
        let stmt = list.iter().next()?;
        let call = stmt.as_call_node()?;
        if call.is_safe_navigation() || call.name().as_slice() != b"[]" {
            return None;
        }
        let receiver = call.receiver()?;
        if receiver.as_local_variable_read_node().is_none()
            && receiver.as_it_local_variable_read_node().is_none()
        {
            return None;
        }
        // A `&blk` is the key argument in whitequark (`block_pass`); Prism keeps it
        // in `block()`.
        let block_pass = match call.block() {
            Some(b) if b.as_block_argument_node().is_some() => Some(b),
            Some(_) => return None,
            None => None,
        };
        match (call.arguments(), block_pass) {
            (None, Some(bp)) => Some((argument, bp)),
            (Some(arguments), None) => {
                let args = arguments.arguments();
                if args.len() != 1 {
                    return None;
                }
                Some((argument, args.iter().next()?))
            }
            _ => None,
        }
    }
}

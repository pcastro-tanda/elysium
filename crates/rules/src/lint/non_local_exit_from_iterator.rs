//! `Lint/NonLocalExitFromIterator`, ported from RuboCop's
//! `lib/rubocop/cop/lint/non_local_exit_from_iterator.rb`.
//!
//! # Ancestor tracking
//!
//! Upstream walks `return_node.each_ancestor(:any_block, :any_def)`
//! outward, stopping at the first ancestor that either settles the
//! question (`scoped_node?`/`define_method?`) or matches
//! (`chained_send?`), and otherwise skipping past an argument-less block to
//! keep looking further out. [`Context::ancestors`] only carries kind and
//! span, not a block's owning `send` node, so this port keeps its own
//! ordered [`Frame`] stack instead: [`Frame::Scoped`] pushed on every
//! [`ruby_ast::NodeKind::DefNode`] (covers both an instance and a singleton
//! method definition -- whitequark's separate `:def`/`:defs` collapse to
//! one Prism kind, distinguished only by an optional `receiver` field,
//! irrelevant here since both count as `any_def_type?`), every
//! [`ruby_ast::NodeKind::LambdaNode`] (Prism's distinct kind for a stabby
//! `-> { }`, which whitequark instead represents as an ordinary `:block`
//! node whose `send` is named `:lambda`), and every literal-block-owning
//! [`ruby_ast::NodeKind::CallNode`] whose own name is `lambda` (matching
//! `BlockNode#lambda?`'s `send_node.method?(:lambda)`, a bare name check
//! with no receiver restriction); [`Frame::Block`] pushed on every other
//! literal-block-owning `CallNode`, precomputed with whether the block has
//! arguments, whether the owning call is `define_method`/
//! `define_singleton_method`, and whether the owning call has a receiver
//! (`chained_send?`'s `(send !nil? ...)`). Both `chained_send?` and
//! `define_method?` match a `send` node type only, never `csend` (Prism's
//! safe-navigation `&.`), so a safe-navigated owning call
//! (`items&.each { return }`, `obj&.define_method(:m) { return }`) is
//! excluded from both checks here too.
//! [`NonLocalExitFromIterator::check`] then walks the stack innermost-first,
//! mirroring the four-way branch in upstream's loop body exactly.
//!
//! # `argument_list.empty?`
//!
//! Whitequark's `argument_list` flattens a block's parameter list down to
//! every actual argument-shaped descendant (`arg`, `optarg`, `restarg`,
//! `kwarg`, `kwoptarg`, `kwrestarg`, `blockarg`, `forward_arg`, and
//! `shadowarg`), so it is non-empty for `|a|`, `|*a|`, `|a:|`, `|&a|`, and
//! even a trailing `|; a|` block-local declaration alone, but empty for
//! `||`/no parameter list at all; for a numblock/itblock it is always
//! non-empty by construction (the node only exists because the block body
//! already referenced `_1`/`it`). [`block_has_arguments`] reproduces this:
//! `BlockNode::parameters()` being a `NumberedParametersNode`/
//! `ItParametersNode` is always "has arguments"; a `BlockParametersNode` has
//! arguments if its own `parameters()` (the real arg list) is present *or*
//! its `locals()` (the `; a` shadow-local list) is non-empty; no
//! `parameters()` field at all means `||`/an implicit empty list, i.e. no
//! arguments.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::BlockNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Non-local exit from iterator, without return value. \
`next`, `break`, `Array#find`, `Array#any?`, etc. is preferred.";

/// One enclosing `any_block`/`any_def` ancestor, innermost last.
#[derive(Debug, Clone, Copy)]
enum Frame {
    /// RuboCop's `scoped_node?` (`any_def_type? || lambda?`): a `def`/`defs`,
    /// a stabby `-> { }` lambda literal, or a block literal whose owning
    /// call is named `lambda`. Unconditionally stops the ancestor walk
    /// without reporting.
    Scoped,
    /// A block literal (`do...end`/`{}`) that is neither scoped nor a
    /// `lambda` call.
    Block {
        /// RuboCop's `!node.argument_list.empty?`.
        has_arguments: bool,
        /// RuboCop's `define_method?(node.send_node)`: not a safe-navigated
        /// (`csend`) call.
        is_define_method: bool,
        /// RuboCop's `chained_send?(node.send_node)`: the owning call has an
        /// explicit receiver and is not safe-navigated (`csend`).
        is_chained: bool,
    },
}

/// Whether a `CallNode` owns a literal block (as opposed to a `&block`
/// argument pass, which is a `BlockArgumentNode` and does not count as
/// `any_block_type?` upstream).
fn owns_literal_block(node: &Node<'_>) -> bool {
    node.as_call_node()
        .is_some_and(|call| call.block().is_some_and(|b| b.as_block_node().is_some()))
}

/// RuboCop-AST's `BlockNode#argument_list.empty?`, negated. See the module
/// doc's "`argument_list.empty?`" section.
fn block_has_arguments(block: &BlockNode<'_>) -> bool {
    match block.parameters() {
        None => false,
        Some(params) => match params.kind() {
            NodeKind::NumberedParametersNode | NodeKind::ItParametersNode => true,
            NodeKind::BlockParametersNode => {
                let Some(block_params) = params.as_block_parameters_node() else { return false };
                block_params.parameters().is_some() || !block_params.locals().is_empty()
            }
            _ => false,
        },
    }
}

/// Checks for non-local exits from iterators without a return value.
///
/// It registers an offense under these conditions:
///
/// * No value is returned,
/// * the block is preceded by a method chain,
/// * the block has arguments,
/// * the method which receives the block is not `define_method`
///   or `define_singleton_method`,
/// * the return is not contained in an inner scope, e.g. a lambda or a
///   method definition.
///
/// # Examples
///
/// ```ruby
/// class ItemApi
///   rescue_from ValidationError do |e| # non-iteration block with arg
///     return { message: 'validation error' } unless e.errors # allowed
///     error_array = e.errors.map do |error| # block with method chain
///       return if error.suppress? # warned
///       return "#{error.param}: invalid" unless error.message # allowed
///       "#{error.param}: #{error.message}"
///     end
///     { message: 'validation error', errors: error_array }
///   end
///
///   def update_items
///     transaction do # block without arguments
///       return unless update_necessary? # allowed
///       find_each do |item| # block without method chain
///         return if item.stock == 0 # false-negative...
///         item.update!(foobar: true)
///       end
///     end
///   end
/// end
/// ```
#[derive(Debug, Clone, Default)]
pub struct NonLocalExitFromIterator {
    /// Enclosing `any_block`/`any_def` ancestors, innermost last.
    stack: Vec<Frame>,
}

impl NonLocalExitFromIterator {
    fn check(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let ret = node.as_return_node().expect("kind matched");
        if ret.arguments().is_some() {
            return;
        }
        for frame in self.stack.iter().rev() {
            match *frame {
                Frame::Scoped | Frame::Block { is_define_method: true, .. } => break,
                Frame::Block { has_arguments: false, .. } => {}
                Frame::Block { is_chained: true, .. } => {
                    ctx.report(&Self::META, ret.keyword_loc().span(), MSG);
                    break;
                }
                Frame::Block { .. } => {}
            }
        }
    }
}

impl Rule for NonLocalExitFromIterator {
    const META: RuleMeta = RuleMeta {
        name: "Lint/NonLocalExitFromIterator",
        department: Department::Lint,
        summary: "Checks for non-local exits from iterators without a return value.",
        explanation: "\
It registers an offense under these conditions:

* No value is returned,
* the block is preceded by a method chain,
* the block has arguments,
* the method which receives the block is not `define_method`
  or `define_singleton_method`,
* the return is not contained in an inner scope, e.g. a lambda or a
  method definition.

```ruby
class ItemApi
  rescue_from ValidationError do |e| # non-iteration block with arg
    return { message: 'validation error' } unless e.errors # allowed
    error_array = e.errors.map do |error| # block with method chain
      return if error.suppress? # warned
      return \"#{error.param}: invalid\" unless error.message # allowed
      \"#{error.param}: #{error.message}\"
    end
    { message: 'validation error', errors: error_array }
  end

  def update_items
    transaction do # block without arguments
      return unless update_necessary? # allowed
      find_each do |item| # block without method chain
        return if item.stock == 0 # false-negative...
        item.update!(foobar: true)
      end
    end
  end
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ReturnNode, NodeKind::CallNode, NodeKind::DefNode, NodeKind::LambdaNode],
        config: &[],
        blind_spots: "\
Upstream's ancestor walk stops looking further out the moment it passes an
argument-less block, *unless* that block is itself scoped or a
`define_method`/`lambda` call -- so a chained, argumented block nested
inside an argument-less, unchained one (`transaction do; find_each do |item|;
return if ...; end; end`) is a documented false-negative on both sides of
this port (the `find_each` block itself is not chained, and the search never
reaches past `transaction`'s empty argument list to find a chained ancestor
further out, because there is none in that example -- see the last spec
example). `chained_send?`/`lambda?`/`define_method?` all match by bare
method name only; `chained_send?`/`define_method?` additionally require a
`send`, not `csend`, owning call, so a safe-navigated one (`items&.each { }`,
`obj&.define_method(:m) { }`) is excluded from both, exactly like upstream.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.stack.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode | NodeKind::LambdaNode => {
                self.stack.push(Frame::Scoped);
            }
            NodeKind::CallNode => {
                if owns_literal_block(node) {
                    let call = node.as_call_node().expect("kind matched");
                    if call.name().as_slice() == b"lambda" {
                        self.stack.push(Frame::Scoped);
                    } else {
                        let block = call
                            .block()
                            .and_then(|b| b.as_block_node())
                            .expect("owns_literal_block checked");
                        self.stack.push(Frame::Block {
                            has_arguments: block_has_arguments(&block),
                            is_define_method: !call.is_safe_navigation()
                                && matches!(
                                    call.name().as_slice(),
                                    b"define_method" | b"define_singleton_method"
                                ),
                            is_chained: call.receiver().is_some() && !call.is_safe_navigation(),
                        });
                    }
                }
            }
            NodeKind::ReturnNode => {
                self.check(node, ctx);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode | NodeKind::LambdaNode => {
                self.stack.pop();
            }
            NodeKind::CallNode if owns_literal_block(node) => {
                self.stack.pop();
            }
            _ => {}
        }
    }
}

//! `Rails/RenderInline`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/render_inline.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeKind};
use ruby_source::Span;

const MSG: &str = "Prefer using a template over inline rendering.";

/// Looks for inline rendering within controller actions.
#[derive(Debug, Clone)]
pub struct RenderInline;

impl Rule for RenderInline {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RenderInline",
        department: Department::Rails,
        summary: "Prefer using a template over inline rendering.",
        explanation: "Looks for inline rendering within controller actions.\n\n```ruby\n# bad\n\
                      class ProductsController < ApplicationController\n  def index\n    \
                      render inline: \"<% products.each do |p| %><p><%= p.name %></p><% end \
                      %>\", type: :erb\n  end\nend\n\n# good\n# app/views/products/index.html.erb\n\
                      # <% products.each do |p| %>\n#   <p><%= p.name %></p>\n# <% end %>\n\n\
                      class ProductsController < ApplicationController\n  def index\n  end\n\
                      end\n```",
        enabled_by_default: false,
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
        if let Some(span) = render_with_inline_option(node) {
            ctx.report(&Self::META, span, MSG);
        }
    }
}

/// `(send nil? :render (hash <(pair {(sym :inline) (str "inline")} _) ...>))`.
fn render_with_inline_option(node: &Node<'_>) -> Option<Span> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"render" || call.receiver().is_some() || call.is_safe_navigation()
    {
        return None;
    }
    // A `&block` argument is a second `send` argument in whitequark.
    if call.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
        return None;
    }
    let arguments = call.arguments()?;
    let mut arguments = arguments.arguments().iter();
    let (Some(argument), None) = (arguments.next(), arguments.next()) else { return None };
    let elements = if let Some(hash) = argument.as_keyword_hash_node() {
        hash.elements()
    } else {
        argument.as_hash_node()?.elements()
    };
    let found = elements.iter().any(|element| {
        let Some(pair) = element.as_assoc_node() else { return false };
        let key = pair.key();
        key.as_symbol_node().is_some_and(|symbol| symbol.unescaped() == b"inline")
            || key.as_string_node().is_some_and(|string| string.unescaped() == b"inline")
    });
    found.then(|| call_span_excluding_block(&call))
}

//! `Rails/MultipleRoutePaths`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/multiple_route_paths.rb` (with its
//! `RoutesHelper` mixin).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str =
    "Use separate routes instead of combining multiple route paths in a single route.";
const HTTP_METHODS: [&[u8]; 5] = [b"get", b"post", b"put", b"patch", b"delete"];

/// Checks for mapping a route with multiple paths, which is deprecated and
/// will be removed in Rails 8.1.
#[derive(Debug, Clone)]
pub struct MultipleRoutePaths {
    /// Spans of the literal blocks of `<_>.routes.draw` calls seen so far
    /// (`RoutesHelper#within_routes?`).
    routes_blocks: Vec<Span>,
}

impl Rule for MultipleRoutePaths {
    const META: RuleMeta = RuleMeta {
        name: "Rails/MultipleRoutePaths",
        department: Department::Rails,
        summary: "Checks for mapping a route with multiple paths, which is deprecated and will \
                  be removed in Rails 8.1.",
        explanation: "Checks for mapping a route with multiple paths, which is deprecated and \
                      will be removed in Rails 8.1.\n\n```ruby\n# bad\nget '/users', \
                      '/other_path', to: 'users#index'\n\n# good\nget '/users', to: \
                      'users#index'\nget '/other_path', to: 'users#index'\n```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { routes_blocks: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if let Some(block) = routes_draw_block(&call) {
            self.routes_blocks.push(block);
        }
        if call.is_safe_navigation() || !HTTP_METHODS.contains(&call.name().as_slice()) {
            return;
        }
        let span = node.span();
        if !self
            .routes_blocks
            .iter()
            .any(|block| block.start <= span.start && span.end <= block.end)
        {
            return;
        }
        // `IGNORED_ARGUMENT_TYPES = %i[array hash]`; a `&block` argument is
        // an argument in whitequark.
        let mut route_paths: Vec<Span> = Vec::new();
        if let Some(arguments) = call.arguments() {
            for argument in &arguments.arguments() {
                if argument.as_array_node().is_none()
                    && argument.as_hash_node().is_none()
                    && argument.as_keyword_hash_node().is_none()
                {
                    route_paths.push(argument.span());
                }
            }
        }
        if let Some(block) = call.block() {
            if block.as_block_argument_node().is_some() {
                route_paths.push(block.span());
            }
        }
        if route_paths.len() < 2 {
            return;
        }

        let call_span = call_span_excluding_block(&call);
        let last = route_paths[route_paths.len() - 1];
        let rest = ctx.text(Span::new(last.end, call_span.end));
        let line_start = ctx.line_span(ctx.line_col(call_span.start).line).start;
        let column = String::from_utf8_lossy(ctx.text(Span::new(line_start, call_span.start)))
            .chars()
            .count();
        let indentation = " ".repeat(column);
        let method = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        let replacement = route_paths
            .iter()
            .map(|path| {
                format!(
                    "{method} {}{}",
                    String::from_utf8_lossy(ctx.text(*path)),
                    String::from_utf8_lossy(rest)
                )
            })
            .collect::<Vec<_>>()
            .join(&format!("\n{indentation}"));
        ctx.report_with_fix(
            &Self::META,
            call_span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(call_span, replacement.into_bytes())],
            },
        );
    }
}

/// The literal block of `(send (send _ :routes) :draw)`.
fn routes_draw_block(call: &ruby_ast::node::CallNode<'_>) -> Option<Span> {
    if call.is_safe_navigation() || call.name().as_slice() != b"draw" || call.arguments().is_some()
    {
        return None;
    }
    let receiver = call.receiver()?;
    let routes = receiver.as_call_node()?;
    if routes.is_safe_navigation()
        || routes.name().as_slice() != b"routes"
        || routes.arguments().is_some()
    {
        return None;
    }
    let block = call.block()?;
    let literal = block.as_block_node()?;
    // numbered-parameter and `it` blocks are not `block` nodes.
    if literal.parameters().is_some_and(|parameters| {
        parameters.as_numbered_parameters_node().is_some()
            || parameters.as_it_parameters_node().is_some()
    }) {
        return None;
    }
    Some(block.location().span())
}

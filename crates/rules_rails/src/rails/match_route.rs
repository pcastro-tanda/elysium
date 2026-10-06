//! `Rails/MatchRoute`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/match_route.rb` (with `RoutesHelper`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::AssocNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::util::{parser_args, plain_block_span, send_span};

const HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete"];

/// Don't use `match` to define any routes unless there is a need to map
/// multiple request types to a single action.
#[derive(Debug, Clone)]
pub struct MatchRoute {
    /// Spans of `routes.draw { }` blocks entered so far.
    routes_blocks: Vec<Span>,
}

impl Rule for MatchRoute {
    const META: RuleMeta = RuleMeta {
        name: "Rails/MatchRoute",
        department: Department::Rails,
        summary: "Don't use `match` to define any routes unless there is a need to map \
                  multiple request types among [:get, :post, :patch, :put, :delete] to a single \
                  action using the `:via` option.",
        explanation: "Identifies places where defining routes with `match` can be replaced with \
                      a specific HTTP method.\n\nDon't use `match` to define any routes unless \
                      there is a need to map multiple request types among [:get, :post, :patch, \
                      :put, :delete] to a single action using the `:via` option.\n\n```ruby\n# \
                      bad\nmatch ':controller/:action/:id'\nmatch 'photos/:id', to: \
                      'photos#show', via: :get\n\n# good\nget ':controller/:action/:id'\nget \
                      'photos/:id', to: 'photos#show'\nmatch 'photos/:id', to: 'photos#show', \
                      via: [:get, :post]\nmatch 'photos/:id', to: 'photos#show', via: :all\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { routes_blocks: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.routes_blocks.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if is_routes_draw(&call) {
            self.routes_blocks.extend(plain_block_span(&call));
        }
        if call.name().as_slice() != b"match"
            || call.receiver().is_some()
            || call.is_safe_navigation()
        {
            return;
        }
        // `(send nil? :match $_ $(hash ...) ?)`
        let arguments = parser_args(&call);
        let (path_node, options_node) = match arguments.as_slice() {
            [path] => (path, None),
            [path, options] if is_hash(options) => (path, Some(options)),
            _ => return,
        };
        let at = node.span().start;
        if !self.routes_blocks.iter().any(|span| span.start <= at && at < span.end) {
            return;
        }
        let effective = if is_hash(path_node) { Some(path_node) } else { options_node };
        let http_method = match effective {
            None => "get".to_owned(),
            Some(options) => match extract_via(options) {
                Some(via) if via.len() == 1 && HTTP_METHODS.contains(&via[0].as_str()) => {
                    via[0].clone()
                }
                _ => return,
            },
        };
        let replacement = replacement(path_node, options_node, ctx);
        ctx.report_with_fix(
            &Self::META,
            send_span(&call),
            format!("Use `{http_method}` instead of `match` to define a route."),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(send_span(&call), replacement.into_bytes())],
            },
        );
    }
}

/// `(send (send _ :routes) :draw)`.
fn is_routes_draw(call: &ruby_ast::node::CallNode<'_>) -> bool {
    if call.name().as_slice() != b"draw" || call.arguments().is_some() || call.is_safe_navigation()
    {
        return false;
    }
    call.receiver().and_then(|r| {
        r.as_call_node().map(|c| {
            c.name().as_slice() == b"routes"
                && c.arguments().is_none()
                && c.block().is_none()
                && !c.is_safe_navigation()
        })
    }) == Some(true)
}

fn is_hash(node: &Node<'_>) -> bool {
    node.as_hash_node().is_some() || node.as_keyword_hash_node().is_some()
}

fn pairs<'pr>(node: &Node<'pr>) -> Vec<AssocNode<'pr>> {
    let elements: Vec<Node<'pr>> = if let Some(hash) = node.as_hash_node() {
        hash.elements().iter().collect()
    } else if let Some(hash) = node.as_keyword_hash_node() {
        hash.elements().iter().collect()
    } else {
        Vec::new()
    };
    elements.iter().filter_map(Node::as_assoc_node).collect()
}

/// A literal's `value` as a string (symbols and strings, the only ones the
/// cop can meaningfully compare).
fn literal_value(node: &Node<'_>) -> Option<String> {
    if let Some(sym) = node.as_symbol_node() {
        Some(String::from_utf8_lossy(sym.unescaped()).into_owned())
    } else {
        node.as_string_node().map(|s| String::from_utf8_lossy(s.unescaped()).into_owned())
    }
}

fn via_pair<'pr>(node: &Node<'pr>) -> Option<AssocNode<'pr>> {
    pairs(node)
        .into_iter()
        .find(|p| p.key().as_symbol_node().is_some_and(|s| s.unescaped() == b"via"))
}

/// `extract_via`; `None` where upstream's `[]` or a crash would make the
/// offense impossible.
fn extract_via(node: &Node<'_>) -> Option<Vec<String>> {
    let Some(pair) = via_pair(node) else { return Some(vec!["get".to_owned()]) };
    let via = pair.value();
    if let Some(array) = via.as_array_node() {
        array.elements().iter().map(|e| literal_value(&e)).collect()
    } else {
        literal_value(&via).map(|v| vec![v])
    }
}

fn replacement(path_node: &Node<'_>, options_node: Option<&Node<'_>>, ctx: &Context<'_>) -> String {
    let text = |span: Span| String::from_utf8_lossy(ctx.text(span)).into_owned();
    let rest = |node: &Node<'_>| {
        let via = via_pair(node).map(|p| p.as_node().span());
        let http_method = extract_via(node).and_then(|v| v.into_iter().next()).unwrap_or_default();
        let options: Vec<String> = pairs(node)
            .iter()
            .filter(|p| Some(p.as_node().span()) != via)
            .map(|p| text(p.as_node().span()))
            .collect();
        (http_method, options)
    };
    if is_hash(path_node) {
        let (http_method, options) = rest(path_node);
        format!("{http_method} {}", options.join(", "))
    } else if let Some(options_node) = options_node {
        let (http_method, options) = rest(options_node);
        if options.is_empty() {
            format!("{http_method} {}", text(path_node.span()))
        } else {
            format!("{http_method} {}, {}", text(path_node.span()), options.join(", "))
        }
    } else {
        format!("get {}", text(path_node.span()))
    }
}

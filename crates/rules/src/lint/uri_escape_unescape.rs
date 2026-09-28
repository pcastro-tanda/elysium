//! `Lint/UriEscapeUnescape`, ported from RuboCop's
//! `lib/rubocop/cop/lint/uri_escape_unescape.rb`.
//!
//! Upstream's node pattern `(send (const ${nil? cbase} :URI) ${:escape
//! :encode :unescape :decode} ...)` requires the receiver to be exactly a
//! bare `URI` or top-level `::URI` constant (rubocop-ast's `{nil? cbase}`
//! shape) -- `Foo::URI.escape(...)` or a `URI` local variable never match.
//! [`ruby_ast::ext::is_bare_or_toplevel_const`] checks that shape; the name
//! is then compared locally, following this codebase's established idiom
//! for the same upstream pattern (see `Lint/UselessAccessModifier`,
//! `Lint/DuplicateMethods`).
//!
//! `top_level` in upstream's block is the pattern's `${nil? cbase}` capture:
//! `nil` for a bare `URI` (no leading `::`), or the (truthy) `cbase` node
//! for `::URI`. A `ConstantReadNode` is always the bare form; a
//! `ConstantPathNode` only reaches this rule (via the shape check above)
//! when it has no parent, i.e. `::URI` -- so the node kind alone tells
//! [`double_colon`] which case it is.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};

/// RuboCop's `ALTERNATE_METHODS_OF_URI_ESCAPE`.
const ALTERNATE_METHODS_OF_URI_ESCAPE: [&str; 3] =
    ["CGI.escape", "URI.encode_www_form", "URI.encode_www_form_component"];

/// RuboCop's `ALTERNATE_METHODS_OF_URI_UNESCAPE`.
const ALTERNATE_METHODS_OF_URI_UNESCAPE: [&str; 3] =
    ["CGI.unescape", "URI.decode_www_form", "URI.decode_www_form_component"];

/// Checks for places where `URI.escape`/`URI.encode`/`URI.unescape`/
/// `URI.decode` can be replaced by more specific, non-obsolete methods.
#[derive(Debug, Clone, Copy, Default)]
pub struct UriEscapeUnescape;

/// rubocop-ast's `(const {nil? cbase} :URI)` shape, restricted to the
/// specific name `URI`. Delegates the shape check to
/// [`ext::is_bare_or_toplevel_const`]; the name comparison stays local
/// since the shared helper only checks shape.
fn is_bare_or_toplevel_uri_const(node: &Node<'_>) -> bool {
    if !ext::is_bare_or_toplevel_const(node) {
        return false;
    }
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"URI")
        }
        NodeKind::ConstantPathNode => node
            .as_constant_path_node()
            .is_some_and(|p| p.name().is_some_and(|n| n.as_slice() == b"URI")),
        _ => unreachable!("ext::is_bare_or_toplevel_const already checked the shape"),
    }
}

/// Whether `node` is a top-level-qualified `::URI` reference (a
/// `ConstantPathNode`, which -- having already passed
/// [`is_bare_or_toplevel_uri_const`]'s shape check -- can only mean it has
/// no parent, i.e. a leading `::` and nothing else) rather than a bare
/// `URI`.
fn double_colon(node: &Node<'_>) -> &'static str {
    if node.kind() == NodeKind::ConstantPathNode {
        "::"
    } else {
        ""
    }
}

impl Rule for UriEscapeUnescape {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UriEscapeUnescape",
        department: Department::Lint,
        summary: "Checks for places where `URI.escape`/`URI.unescape` (and their aliases) can be \
                  replaced by more specific, non-obsolete methods.",
        explanation: "\
Identifies places where `URI.escape` can be replaced by `CGI.escape`,
`URI.encode_www_form`, or `URI.encode_www_form_component` depending on your
specific use case. Also this cop identifies places where `URI.unescape` can
be replaced by `CGI.unescape`, `URI.decode_www_form`, or
`URI.decode_www_form_component` depending on your specific use case.

```ruby
# bad
URI.escape('http://example.com')
URI.encode('http://example.com')

# good
CGI.escape('http://example.com')
URI.encode_uri_component(uri) # Since Ruby 3.1
URI.encode_www_form([['example', 'param'], ['lang', 'en']])
URI.encode_www_form(page: 10, locale: 'en')
URI.encode_www_form_component('http://example.com')

# bad
URI.unescape(enc_uri)
URI.decode(enc_uri)

# good
CGI.unescape(enc_uri)
URI.decode_uri_component(uri) # Since Ruby 3.1
URI.decode_www_form(enc_uri)
URI.decode_www_form_component(enc_uri)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Only matches a receiver that is exactly a bare `URI` or top-level `::URI` constant, per upstream's
`(const {nil? cbase} :URI)` pattern; a re-exported or aliased `URI` (e.g. `Foo::URI.escape(...)` or
`my_uri = URI; my_uri.escape(...)`) is not flagged, matching upstream.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        check(&call, ctx);
    }
}

/// RuboCop's `on_send`, guarded by `RESTRICT_ON_SEND`.
fn check(call: &CallNode<'_>, ctx: &mut Context<'_>) {
    let method = call.name();
    let method_bytes = method.as_slice();
    if !matches!(method_bytes, b"escape" | b"encode" | b"unescape" | b"decode") {
        return;
    }
    let Some(receiver) = call.receiver() else { return };
    if !is_bare_or_toplevel_uri_const(&receiver) {
        return;
    }

    let replacements = if matches!(method_bytes, b"escape" | b"encode") {
        ALTERNATE_METHODS_OF_URI_ESCAPE
    } else {
        ALTERNATE_METHODS_OF_URI_UNESCAPE
    };

    let uri_method =
        format!("{}URI.{}", double_colon(&receiver), String::from_utf8_lossy(method_bytes));
    let message = format!(
        "`{uri_method}` method is obsolete and should not be used. Instead, use `{}`, `{}` or \
         `{}` depending on your specific use case.",
        replacements[0], replacements[1], replacements[2]
    );

    ctx.report(&UriEscapeUnescape::META, call.as_node().span(), message);
}

//! `Lint/UriRegexp`, ported from RuboCop's
//! `lib/rubocop/cop/lint/uri_regexp.rb`.
//!
//! Upstream's `RESTRICT_ON_SEND = %i[regexp]` plus `on_send` (no `on_csend`
//! alias) means only a plain `.regexp` call is checked, never `&.regexp`;
//! the receiver must additionally be exactly a bare `URI` or top-level
//! `::URI` constant (rubocop-ast's `(const {cbase nil?} :URI)` shape), so
//! `Foo::URI.regexp(...)` or a `URI` local variable are never flagged.
//! [`ruby_ast::ext::is_bare_or_toplevel_const`] checks that shape, following
//! this codebase's established idiom for the same upstream pattern (see
//! `Lint/UriEscapeUnescape`).
//!
//! Upstream's `parser = target_ruby_version >= 3.4 ? 'RFC2396_PARSER' :
//! 'DEFAULT_PARSER'` is mirrored via `options.target_ruby_version()`, and
//! `argument = node.first_argument ? "(#{node.first_argument.source})" : ''`
//! -- note only the *first* argument's source is used, matching upstream
//! literally even though `URI.regexp` accepts a single schemes argument in
//! practice.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};

/// Use `URI::DEFAULT_PARSER.make_regexp` (or `URI::RFC2396_PARSER` on Ruby
/// 3.4+) instead of `URI.regexp`.
#[derive(Debug, Clone, Copy)]
pub struct UriRegexp {
    target_ruby_version: f32,
}

/// rubocop-ast's `(const {cbase nil?} :URI)` shape, restricted to the
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

impl Rule for UriRegexp {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UriRegexp",
        department: Department::Lint,
        summary: "Identifies places where `URI.regexp` is obsolete and should not be used.",
        explanation: "\
Identifies places where `URI.regexp` is obsolete and should not be used.

For Ruby 3.3 or lower, use `URI::DEFAULT_PARSER.make_regexp`.
For Ruby 3.4 or higher, use `URI::RFC2396_PARSER.make_regexp`.

NOTE: If you need to support both Ruby 3.3 and lower as well as Ruby 3.4 and higher,
consider manually changing the code as follows:

```ruby
defined?(URI::RFC2396_PARSER) ? URI::RFC2396_PARSER : URI::DEFAULT_PARSER
```

```ruby
# bad
URI.regexp('http://example.com')

# good - Ruby 3.3 or lower
URI::DEFAULT_PARSER.make_regexp('http://example.com')

# good - Ruby 3.4 or higher
URI::RFC2396_PARSER.make_regexp('http://example.com')
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Only matches a receiver that is exactly a bare `URI` or top-level `::URI` constant, per upstream's
`(const {cbase nil?} :URI)` pattern; a re-exported or aliased `URI` (e.g. `Foo::URI.regexp(...)` or
`my_uri = URI; my_uri.regexp(...)`) is not flagged, matching upstream. Only the first argument's
source is used when rebuilding the replacement call, matching upstream's
`node.first_argument.source` (any further arguments are dropped from the suggested replacement).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        if call.name().as_slice() != b"regexp" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !is_bare_or_toplevel_uri_const(&receiver) {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };

        let parser =
            if self.target_ruby_version >= 3.4 { "RFC2396_PARSER" } else { "DEFAULT_PARSER" };
        let argument = call
            .arguments()
            .and_then(|args| args.arguments().iter().next())
            .map(|first| format!("({})", String::from_utf8_lossy(ctx.text(first.span()))));

        let receiver_source = String::from_utf8_lossy(ctx.text(receiver.span()));
        let preferred_method =
            format!("{receiver_source}::{parser}.make_regexp{}", argument.as_deref().unwrap_or(""));
        let current = String::from_utf8_lossy(ctx.text(call.as_node().span())).into_owned();
        let message = format!(
            "`{current}` is obsolete and should not be used. Instead, use `{preferred_method}`."
        );

        ctx.report_with_fix(
            &Self::META,
            message_loc.span(),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(call.as_node().span(), preferred_method.into_bytes())],
            },
        );
    }
}

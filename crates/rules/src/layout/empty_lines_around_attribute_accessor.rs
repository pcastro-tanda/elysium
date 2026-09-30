//! `Layout/EmptyLinesAroundAttributeAccessor`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_lines_around_attribute_accessor.rb`.
//!
//! # Traversal shape
//!
//! Upstream's `on_send` inspects one `attr_*` call at a time and asks for
//! its `right_sibling` within whitequark's flattened `children` array,
//! special-casing `node.parent.if_type?` to stop a lone `if`/`unless`
//! branch statement from spilling onto the *other* branch's body (since
//! whitequark elides the `begin` wrapper for a single-statement body, that
//! statement's `parent` is the `if` node itself, and a generic
//! `right_sibling` would otherwise walk into the sibling branch's slot).
//!
//! Prism never elides: every body -- single statement or not -- is a
//! [`NodeKind::StatementsNode`]. This port therefore walks each
//! `StatementsNode`'s own `body` list directly instead of re-deriving one
//! statement's sibling from its ancestors: the "next" statement is simply
//! the following item in that same list, or `None` past the last one. This
//! also makes the `if_type?` special case unnecessary -- a lone branch
//! statement's `StatementsNode` has no further item regardless of what
//! follows the enclosing `if`, matching upstream's outcome without needing
//! upstream's guard.
//!
//! # `attribute_accessor?`
//!
//! `rubocop-ast`'s `SendNode#attribute_accessor?` pattern matches a
//! receiver-less `attr_reader`/`attr_writer`/`attr_accessor`/`attr` call
//! with any (including zero) arguments -- so a bare `attr` used only as
//! another call's receiver (`attr.foo`) matches it too, and since that
//! `attr` call's own "sibling" (the method-name slot of the outer call)
//! isn't a node, [`is_attribute_accessor_call`]/the `StatementsNode`-only
//! traversal above never even reaches it: it is not itself a body
//! statement. [`is_attribute_accessor_call`] mirrors upstream's pattern by
//! only checking the call's own receiver/name.
//!
//! # `next_line_enable_directive_comment?`
//!
//! Upstream checks `DirectiveComment#enabled?`, true for any `# rubocop:
//! enable ...` comment regardless of which cops it names. [`enable_directive_at`]
//! mirrors that: it only checks the directive's kind and line, not its cop
//! list.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_directives::DirectiveKind;

const MSG: &str = "Add an empty line after attribute accessor.";

/// Keep blank lines around attribute accessors.
#[derive(Debug, Clone)]
pub struct EmptyLinesAroundAttributeAccessor {
    /// `AllowAliasSyntax`.
    allow_alias_syntax: bool,
    /// `AllowedMethods`.
    allowed_methods: Vec<String>,
}

impl EmptyLinesAroundAttributeAccessor {
    /// RuboCop's `allow_alias?`.
    fn allow_alias(&self, next: &Node<'_>) -> bool {
        self.allow_alias_syntax
            && matches!(next.kind(), NodeKind::AliasMethodNode | NodeKind::AliasGlobalVariableNode)
    }

    /// RuboCop's `attribute_or_allowed_method?`.
    fn attribute_or_allowed_method(&self, next: &Node<'_>) -> bool {
        let Some(call) = next.as_call_node() else { return false };
        is_attribute_accessor_call(&call)
            || self.allowed_methods.iter().any(|m| m.as_bytes() == call.name().as_slice())
    }

    /// RuboCop's `require_empty_line?`.
    fn require_empty_line(&self, next: Option<&Node<'_>>) -> bool {
        let Some(next) = next else { return false };
        !self.allow_alias(next) && !self.attribute_or_allowed_method(next)
    }
}

impl Rule for EmptyLinesAroundAttributeAccessor {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLinesAroundAttributeAccessor",
        department: Department::Layout,
        summary: "Keep blank lines around attribute accessors.",
        explanation: "\
Checks for a newline after an attribute accessor or a group of them.
`alias` syntax and `alias_method`, `public`, `protected`, and `private`
methods are allowed by default. These are customizable with
`AllowAliasSyntax` and `AllowedMethods` options.

```ruby
# bad
attr_accessor :foo
def do_something
end

# good
attr_accessor :foo

def do_something
end

# good
attr_accessor :foo
attr_reader :bar
attr_writer :baz
attr :qux

def do_something
end
```

With `AllowAliasSyntax: true` (default):

```ruby
# good
attr_accessor :foo
alias :foo? :foo

def do_something
end
```

With `AllowAliasSyntax: false`:

```ruby
# bad
attr_accessor :foo
alias :foo? :foo

def do_something
end

# good
attr_accessor :foo

alias :foo? :foo

def do_something
end
```

With `AllowedMethods: ['private']`:

```ruby
# good
attr_accessor :foo
private :foo

def do_something
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode],
        config: &[
            ConfigOption {
                name: "AllowAliasSyntax",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether an `alias` immediately following an accessor is exempt from the \
                      blank-line requirement.",
            },
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&[
                    "alias_method",
                    "public",
                    "protected",
                    "private",
                ]),
                allowed: &[],
                doc: "Method names that may immediately follow an accessor without a blank line.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_alias_syntax: options.bool("AllowAliasSyntax"),
            allowed_methods: options.str_list("AllowedMethods"),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(stmts) = node.as_statements_node() else { return };
        let body = stmts.body();
        let mut items = body.iter().peekable();
        while let Some(item) = items.next() {
            let Some(call) = item.as_call_node() else { continue };
            if !is_attribute_accessor_call(&call) {
                continue;
            }

            let node_span = call_span_excluding_block(&call);
            let last_line = ctx.last_line(node_span);
            let next_physical = last_line + 1;
            if next_physical > ctx.line_count() || is_blank_line(ctx, next_physical) {
                continue;
            }
            let directive_at_next = enable_directive_at(ctx, next_physical);
            if directive_at_next {
                let after = next_physical + 1;
                if after > ctx.line_count() || is_blank_line(ctx, after) {
                    continue;
                }
            }

            if !self.require_empty_line(items.peek()) {
                continue;
            }

            let insert_range = if directive_at_next {
                ctx.whole_lines(ctx.line_span(next_physical))
            } else {
                ctx.whole_lines(node_span)
            };
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::insert(insert_range.end, b"\n".as_slice())],
            };
            ctx.report_with_fix(&Self::META, node_span, MSG, fix);
        }
    }
}

/// RuboCop-AST's `SendNode#attribute_accessor?`: a receiver-less
/// `attr_reader`/`attr_writer`/`attr_accessor`/`attr` call, with any
/// (including zero) arguments.
fn is_attribute_accessor_call(call: &CallNode<'_>) -> bool {
    call.receiver().is_none()
        && matches!(
            call.name().as_slice(),
            b"attr_reader" | b"attr_writer" | b"attr_accessor" | b"attr"
        )
}

/// True when `line` starts a `# rubocop:enable ...` directive comment
/// (any cop list), matching `DirectiveComment#enabled?`.
fn enable_directive_at(ctx: &Context<'_>, line: u32) -> bool {
    ctx.directives().directives().iter().any(|d| d.line == line && d.kind == DirectiveKind::Enable)
}

/// `String#blank?` as applied to one physical line.
fn is_blank_line(ctx: &Context<'_>, line: u32) -> bool {
    ctx.line_text(line).iter().all(u8::is_ascii_whitespace)
}

//! `Naming/MethodName`, ported from RuboCop's
//! `lib/rubocop/cop/naming/method_name.rb`, sharing the `ConfigurableNaming`
//! style regexes and identifier/pattern lists with [`super::variable_name`]
//! via [`super::configurable_naming`].
//!
//! # Reported ranges
//!
//! Upstream reports a `def` on its name, a literal method name (`alias`,
//! `alias_method`, `Struct.new`/`Data.define` members) on the literal itself,
//! and the remaining call forms (`define_method`, `attr_*`) on
//! `range_position`: from one past the selector to the end of the call
//! (`:foo, :bar` -- the whole argument list). Because RuboCop's
//! `add_offense` drops a second offense on an identical range, an
//! `attr_accessor :fooBar, :bazQux` yields one style offense; the forbidden
//! branch likewise reports once, on the *last* argument (upstream
//! `attrs.last.last`), whichever argument was forbidden.
//!
//! # Class emitter methods
//!
//! `def self.Model(...)` next to `class Model` is accepted
//! (`ConfigurableFormatting#class_emitter_method?`): from the singleton
//! `def`, walk out through enclosing singleton `def`s and look for a
//! `class` child of the surrounding body whose name reads the same.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, NodeInfo, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::node::{CallNode, StringNode, SymbolNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::configurable_naming::{identifier_matches, matches_style, pattern_matches, Style};

/// Makes sure that all methods use the configured style, `snake_case` or
/// camelCase, for their names.
#[derive(Debug, Clone)]
pub struct MethodName {
    style: Style,
    allowed_patterns: Vec<Regex>,
    forbidden_identifiers: Vec<String>,
    forbidden_patterns: Vec<Regex>,
}

impl Rule for MethodName {
    const META: RuleMeta = RuleMeta {
        name: "Naming/MethodName",
        department: Department::Naming,
        summary: "Makes sure that all methods use the configured style, snake_case or camelCase, for their names.",
        explanation: "\
Method names matching `AllowedPatterns` are always allowed, and
`ForbiddenIdentifiers`/`ForbiddenPatterns` are always flagged. Operator
methods are never checked.

```ruby
# EnforcedStyle: snake_case (default)

# bad
def fooBar; end

# good
def foo_bar; end
```

```ruby
# EnforcedStyle: camelCase

# bad
def foo_bar; end

# good
def fooBar; end
```

```ruby
# ForbiddenIdentifiers: ['def', 'super']

# bad
def def; end
def super; end
```

```ruby
# ForbiddenPatterns: ['_v1\\z', '_gen1\\z']

# bad
def release_v1; end
def api_gen1; end
```

```ruby
# AllowedPatterns: ['\\AonSelectionBulkChange\\z']

# good
def onSelectionBulkChange(arg); end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::DefNode, NodeKind::CallNode, NodeKind::AliasMethodNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("snake_case"),
                allowed: &["snake_case", "camelCase"],
                doc: "Naming style method names must follow.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Regexps; a method name matching one is never checked.",
            },
            ConfigOption {
                name: "ForbiddenIdentifiers",
                default: ConfigDefault::StrList(&["__id__", "__send__"]),
                allowed: &[],
                doc: "Method names that are always flagged.",
            },
            ConfigOption {
                name: "ForbiddenPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Regexps; a method name matching one is always flagged.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            style: Style::parse(options.style("EnforcedStyle")?),
            allowed_patterns: compile(&options.str_list("AllowedPatterns")),
            forbidden_identifiers: options.str_list("ForbiddenIdentifiers"),
            forbidden_patterns: compile(&options.str_list("ForbiddenPatterns")),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode => self.on_def(node, ctx),
            NodeKind::CallNode => {
                if let Some(call) = node.as_call_node() {
                    self.on_call(&call, ctx);
                }
            }
            NodeKind::AliasMethodNode => {
                if let Some(alias) = node.as_alias_method_node() {
                    let new_name = alias.new_name();
                    if let Some(sym) = new_name.as_symbol_node() {
                        self.handle_method_name(sym.unescaped(), new_name.span(), ctx);
                    }
                }
            }
            _ => {}
        }
    }
}

impl MethodName {
    fn on_def(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        let name = def.name().as_slice();
        if is_operator(name) || pattern_matches(&self.allowed_patterns, name) {
            return;
        }
        let name_span = def.name_loc().span();
        if self.forbidden(name) {
            Self::report_forbidden(ctx, name_span, name);
        } else if !(matches_style(self.style, name)
            || def.receiver().is_some() && class_emitter_method(name, ctx))
        {
            self.report_style(ctx, name_span);
        }
    }

    fn on_call(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let method = call.name().as_slice();
        match method {
            b"define_method" | b"define_singleton_method" => {
                let Some(first) = first_argument(call) else { return };
                if let Some(name) = literal_name(&first) {
                    self.handle_method_name(name.name(), range_position(call), ctx);
                }
            }
            b"new" if receiver_is(call, "Struct") => {
                let Some(arguments) = call.arguments() else { return };
                let list = arguments.arguments();
                let skip =
                    usize::from(list.iter().next().is_some_and(|a| a.as_string_node().is_some()));
                for arg in list.iter().skip(skip) {
                    if let Some(name) = literal_name(&arg) {
                        self.handle_method_name(name.name(), arg.span(), ctx);
                    }
                }
            }
            b"define" if receiver_is(call, "Data") => {
                let Some(arguments) = call.arguments() else { return };
                for arg in &arguments.arguments() {
                    if let Some(name) = literal_name(&arg) {
                        self.handle_method_name(name.name(), arg.span(), ctx);
                    }
                }
            }
            b"alias_method" => {
                let Some(arguments) = call.arguments() else { return };
                let list = arguments.arguments();
                if list.iter().count() != 2 {
                    return;
                }
                let Some(first) = list.iter().next() else { return };
                if let Some(name) = literal_name(&first) {
                    self.handle_method_name(name.name(), first.span(), ctx);
                }
            }
            b"attr" | b"attr_reader" | b"attr_writer" | b"attr_accessor"
                if call.receiver().is_none() =>
            {
                self.handle_attr_accessor(call, ctx);
            }
            _ => {}
        }
    }

    /// Upstream `handle_attr_accessor`: one style offense at most (identical
    /// ranges are dropped by RuboCop), forbidden names reported on the last
    /// argument.
    fn handle_attr_accessor(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let Some(arguments) = call.arguments() else { return };
        let list = arguments.arguments();
        let Some(last_arg) = list.iter().last() else { return };
        let mut style_reported = false;
        let mut forbidden_reported = false;
        for arg in &list {
            let Some(literal) = literal_name(&arg) else { continue };
            let name = literal.name();
            if pattern_matches(&self.allowed_patterns, name) {
                continue;
            }
            if self.forbidden(name) {
                if !forbidden_reported {
                    forbidden_reported = true;
                    let last_literal = literal_name(&last_arg);
                    let identifier = last_literal.as_ref().map_or(name, Literal::name);
                    Self::report_forbidden(ctx, last_arg.span(), identifier);
                }
            } else if !style_reported && !matches_style(self.style, name) {
                style_reported = true;
                self.report_style(ctx, range_position(call));
            }
        }
    }

    /// Upstream `handle_method_name`.
    fn handle_method_name(&self, name: &[u8], span: Span, ctx: &mut Context<'_>) {
        if pattern_matches(&self.allowed_patterns, name) {
            return;
        }
        if self.forbidden(name) {
            Self::report_forbidden(ctx, span, name);
        } else if !is_operator(name) && !matches_style(self.style, name) {
            self.report_style(ctx, span);
        }
    }

    fn forbidden(&self, name: &[u8]) -> bool {
        identifier_matches(&self.forbidden_identifiers, name)
            || pattern_matches(&self.forbidden_patterns, name)
    }

    fn report_style(&self, ctx: &mut Context<'_>, span: Span) {
        ctx.report(
            &Self::META,
            span,
            format!("Use {} for method names.", self.style.config_name()),
        );
    }

    fn report_forbidden(ctx: &mut Context<'_>, span: Span, name: &[u8]) {
        ctx.report(
            &Self::META,
            span,
            format!(
                "`{}` is forbidden, use another method name instead.",
                String::from_utf8_lossy(name)
            ),
        );
    }
}

fn compile(patterns: &[String]) -> Vec<Regex> {
    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
}

/// rubocop-ast `operator_method?` / the cop's `OPERATOR_METHODS`: every
/// operator method name starts with a non-identifier byte.
fn is_operator(name: &[u8]) -> bool {
    !name.first().is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80)
}

fn first_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    call.arguments()?.arguments().iter().next()
}

/// A plain (non-interpolated) symbol or string literal naming a method.
enum Literal<'pr> {
    Sym(SymbolNode<'pr>),
    Str(StringNode<'pr>),
}

impl Literal<'_> {
    fn name(&self) -> &[u8] {
        match self {
            Self::Sym(sym) => sym.unescaped(),
            Self::Str(string) => string.unescaped(),
        }
    }
}

fn literal_name<'pr>(node: &Node<'pr>) -> Option<Literal<'pr>> {
    if let Some(sym) = node.as_symbol_node() {
        Some(Literal::Sym(sym))
    } else {
        node.as_string_node().map(Literal::Str)
    }
}

fn receiver_is(call: &CallNode<'_>, name: &str) -> bool {
    call.receiver().is_some_and(|receiver| {
        is_bare_or_toplevel_const(&receiver) && const_name(&receiver).as_deref() == Some(name)
    })
}

/// Upstream `range_position`: one past the selector through the end of the
/// call (excluding any block).
fn range_position(call: &CallNode<'_>) -> Span {
    let end = ruby_ast::ext::call_span_excluding_block(call).end;
    match call.message_loc() {
        Some(selector) => Span::new((selector.span().end + 1).min(end), end),
        None => ruby_ast::ext::call_span_excluding_block(call),
    }
}

/// `ConfigurableFormatting#class_emitter_method?` for a singleton `def`
/// whose name failed the style check: is there a `class <name>` among the
/// statements surrounding the outermost enclosing singleton `def`?
fn class_emitter_method(name: &[u8], ctx: &Context<'_>) -> bool {
    let ancestors = ctx.ancestors();
    // Innermost first: skip the statement lists and singleton defs we are
    // nested in; the first other statement list is the surrounding body.
    let mut body: Option<&NodeInfo> = None;
    let mut iter = ancestors.iter().rev().peekable();
    while let Some(info) = iter.next() {
        match info.kind {
            NodeKind::StatementsNode => {
                if iter.peek().is_some_and(|parent| parent.kind == NodeKind::DefNode) {
                    continue;
                }
                body = Some(info);
                break;
            }
            NodeKind::DefNode => {}
            _ => break,
        }
    }
    let Some(body) = body else { return false };
    let Some(statements) = node_at(&ctx.parsed().root(), NodeKind::StatementsNode, body.span)
        .and_then(|n| n.as_statements_node())
    else {
        return false;
    };
    statements.body().iter().any(|child| {
        child.as_class_node().is_some_and(|class| ctx.text(class.constant_path().span()) == name)
    })
}

fn node_at<'pr>(root: &Node<'pr>, kind: NodeKind, span: Span) -> Option<Node<'pr>> {
    if root.kind() == kind && root.span() == span {
        return Some(*root);
    }
    if !root.span().contains(span) {
        return None;
    }
    let mut found = None;
    ruby_ast::for_each_child(root, |child| {
        if found.is_none() {
            found = node_at(child, kind, span);
        }
    });
    found
}

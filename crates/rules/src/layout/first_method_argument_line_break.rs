//! `Layout/FirstMethodArgumentLineBreak`, ported from RuboCop's
//! `lib/rubocop/cop/layout/first_method_argument_line_break.rb` plus the
//! `FirstElementLineBreak` and `AllowedMethods` mixins it includes.
//!
//! RuboCop aliases `on_csend`/`on_super` to the same `on_send` handler;
//! Prism unifies regular/safe-navigation calls under `CallNode` (so no
//! separate `on_csend` is needed) but keeps `super(...)`/`super ...` as its
//! own `SuperNode` (bare `super` with no parens/args is a distinct
//! `ForwardingSuperNode`, which upstream's `on_zsuper` -- never aliased here
//! -- would handle, so it is not checked, matching upstream).
//!
//! Prism's `KeywordHashNode` (braceless keyword arguments) is always
//! brace-less by construction -- a braced trailing hash argument parses as
//! an ordinary `HashNode` instead -- so RuboCop's
//! `last_arg&.hash_type? && !last_arg&.braces?` guard for exploding a
//! trailing implicit-hash argument into its key/value pairs simplifies to
//! "the last argument is a `KeywordHashNode`".

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};

/// RuboCop's `MSG`.
const MSG: &str =
    "Add a line break before the first argument of a multi-line method argument list.";

/// Checks for a line break before the first argument in a multi-line method call.
#[derive(Debug, Clone)]
pub struct FirstMethodArgumentLineBreak {
    allow_multiline_final_element: bool,
    /// `AllowedMethods`, plus the deprecated `IgnoredMethods`/`ExcludedMethods` aliases merged in.
    allowed_methods: Vec<String>,
}

impl Rule for FirstMethodArgumentLineBreak {
    const META: RuleMeta = RuleMeta {
        name: "Layout/FirstMethodArgumentLineBreak",
        department: Department::Layout,
        summary: "Checks for a line break before the first argument in a multi-line method call.",
        explanation: "\
```ruby
# bad
method(foo, bar,
  baz)

# good
method(
  foo, bar,
  baz)

# ignored
method foo, bar,
  baz
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::SuperNode],
        config: &[
            ConfigOption {
                name: "AllowMultilineFinalElement",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether the last argument is allowed to start a new, multi-line \
                      element without triggering this cop.",
            },
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method names that are never checked. The deprecated \
                      `IgnoredMethods`/`ExcludedMethods` aliases are merged in too.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mut allowed_methods = options.str_list("AllowedMethods");
        allowed_methods.extend(options.str_list("IgnoredMethods"));
        allowed_methods.extend(options.str_list("ExcludedMethods"));
        Ok(Self {
            allow_multiline_final_element: options.bool("AllowMultilineFinalElement"),
            allowed_methods,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (node_span, method_name, args): (Span, Vec<u8>, Vec<Node<'_>>) = match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                let args =
                    call.arguments().map_or_else(Vec::new, |a| a.arguments().iter().collect());
                (call.as_node().span(), call.name().as_slice().to_vec(), args)
            }
            NodeKind::SuperNode => {
                let sup = node.as_super_node().expect("kind matched");
                let args =
                    sup.arguments().map_or_else(Vec::new, |a| a.arguments().iter().collect());
                (sup.as_node().span(), b"super".to_vec(), args)
            }
            _ => return,
        };

        if self.allowed_methods.iter().any(|m| m.as_bytes() == method_name.as_slice()) {
            return;
        }

        let mut children: Vec<Span> = args.iter().map(ruby_ast::NodeExt::span).collect();
        if let Some(last) = args.last() {
            if let Some(kw) = last.as_keyword_hash_node() {
                children.pop();
                children.extend(kw.elements().iter().map(|n| n.span()));
            }
        }

        check_method_line_break(ctx, node_span, &children, self.allow_multiline_final_element);
    }
}

/// RuboCop's `FirstElementLineBreak#check_method_line_break`.
fn check_method_line_break(
    ctx: &mut Context<'_>,
    node_span: Span,
    children: &[Span],
    ignore_last: bool,
) {
    if children.is_empty() {
        return;
    }
    if !method_uses_parens(ctx, node_span, children[0]) {
        return;
    }
    check_children_line_break(ctx, node_span, children, ignore_last);
}

/// RuboCop's `FirstElementLineBreak#method_uses_parens?`: slices `node`'s own
/// first line up to `limit`'s own column (on `limit`'s own line, which may
/// differ from `node`'s), and checks the trimmed prefix ends in `(`.
fn method_uses_parens(ctx: &Context<'_>, node_span: Span, limit: Span) -> bool {
    let node_line = ctx.line_col(node_span.start).line;
    let limit_line = ctx.line_col(limit.start).line;
    let limit_line_start = ctx.line_span(limit_line).start;
    let col = usize::try_from(limit.start - limit_line_start).unwrap_or(usize::MAX);
    trimmed_prefix_ends_with(ctx.line_text(node_line), col, b'(')
}

/// `prefix[0...col]`, trailing-whitespace-trimmed, ends in `byte`.
fn trimmed_prefix_ends_with(line: &[u8], col: usize, byte: u8) -> bool {
    let col = col.min(line.len());
    let mut i = col;
    while i > 0 && is_ruby_whitespace(line[i - 1]) {
        i -= 1;
    }
    i > 0 && line[i - 1] == byte
}

/// RuboCop's `FirstElementLineBreak#check_children_line_break`, with the
/// default `start: node`.
fn check_children_line_break(
    ctx: &mut Context<'_>,
    node_span: Span,
    children: &[Span],
    ignore_last: bool,
) {
    if children.is_empty() {
        return;
    }

    let line = ctx.line_col(node_span.start).line;

    let min = first_by_line(ctx, children);
    if line != ctx.line_col(min.start).line {
        return;
    }

    let max_line = children
        .iter()
        .map(|&c| if ignore_last { ctx.line_col(c.start).line } else { ctx.last_line(c) })
        .max()
        .expect("non-empty");
    if line == max_line {
        return;
    }

    let fix = Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::insert(min.start, b"\n".to_vec())],
    };
    ctx.report_with_fix(&FirstMethodArgumentLineBreak::META, min, MSG, fix);
}

/// RuboCop's `first_by_line`: `nodes.min_by(&:first_line)`, which keeps the
/// first element reached on ties.
fn first_by_line(ctx: &Context<'_>, children: &[Span]) -> Span {
    let mut min = children[0];
    let mut min_line = ctx.line_col(min.start).line;
    for &c in &children[1..] {
        let l = ctx.line_col(c.start).line;
        if l < min_line {
            min = c;
            min_line = l;
        }
    }
    min
}

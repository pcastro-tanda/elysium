//! `Style/StringConcatenation`, ported from RuboCop's
//! `lib/rubocop/cop/style/string_concatenation.rb`.
//!
//! # Approach
//!
//! RuboCop's `on_send` fires once per qualifying `+` call anywhere in the
//! tree and climbs `node.parent` while it is itself a `+` call to find the
//! chain's topmost node, then reports (and, once, corrects) that topmost
//! node. This engine visits every node exactly once top-down instead of
//! letting rules walk `.parent`, so the climb is inverted: [`enter`] records
//! every `+` call's span in `self.plus_spans` as it is visited (ancestors are
//! always visited before descendants), then a node is topmost exactly when
//! its own effective parent (its immediate ancestor, skipping through the
//! `ArgumentsNode` wrapper Prism -- unlike whitequark -- interposes between a
//! call and its arguments) is *not* itself a recorded `+` call. Once a node
//! is confirmed topmost, [`collect_chain`] walks *down* through the whole
//! `+`-call subtree once (mirroring RuboCop's `collect_parts`) to gather the
//! leaf parts and decide, in the same pass, whether any part of the chain
//! individually satisfies `string_concatenation?` and is not
//! `line_end_concatenation?`-exempt.
//!
//! `corrected_ancestor?` (RuboCop's guard against clobbering an
//! already-corrected overlapping range within the same autocorrect pass) has
//! no counterpart here: the fix engine's `apply_fixes` already skips a fix
//! whose edits overlap an already-accepted one, and a `+` chain nested inside
//! a non-`+` part of an outer chain (e.g. a ternary branch) is only exposed
//! once the outer chain's own correction has been applied and the file
//! re-parsed -- exactly the effect RuboCop gets from `expect_correction`'s
//! default `loop: true`, and what this fixture harness's `fix_file` provides.

use std::collections::HashSet;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Prefer string interpolation to string concatenation.";

/// RuboCop's `Mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Aggressive,
    Conservative,
}

/// Checks for places where string concatenation can be replaced with string
/// interpolation.
#[derive(Debug, Clone)]
pub struct StringConcatenation {
    mode: Mode,
    /// Spans of every `+` call visited so far in the current file, used to
    /// recognize a node's effective parent as another `+` call without
    /// needing that parent's own `Node`.
    plus_spans: HashSet<Span>,
}

impl Rule for StringConcatenation {
    const META: RuleMeta = RuleMeta {
        name: "Style/StringConcatenation",
        department: Department::Style,
        summary: "Checks for places where string concatenation can be replaced with string \
                  interpolation.",
        explanation: "\
```ruby
# bad
email_with_name = user.name + ' <' + user.email + '>'
Pathname.new('/') + 'test'

# good
email_with_name = \"#{user.name} <#{user.email}>\"
email_with_name = format('%s <%s>', user.name, user.email)
\"#{Pathname.new('/')}test\"

# accepted, line-end concatenation
name = 'First' +
  'Last'
```

With `Mode: conservative`, only a `+` whose left-hand side (the receiver) is \
                      a string literal is flagged; `Mode: aggressive` (the default) flags a `+` \
                      with a string literal on either side.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "Mode",
            default: ConfigDefault::Str("aggressive"),
            allowed: &["aggressive", "conservative"],
            doc: "`aggressive` flags any `+` with a string literal on either side; \
                  `conservative` only flags a `+` whose receiver is a string literal.",
        }],
        blind_spots: "\
- A multi-statement `#{a; b}` interpolation hole and a backtick `` ` `` \
          (x)string part are not unwrapped for re-quoting the way a plain string or a \
          single-expression interpolation hole is; both fall back to re-embedding their raw \
          source, matching RuboCop's own `else` branch for any non-`str`/`dstr`/`begin` part.
- A right-associative chain built with explicit parentheses on the argument \
          side (`a + (b + c)`) is vanishingly rare in practice and not verified against \
          RuboCop's real `.parent`-climbing behavior beyond the `ArgumentsNode` wrapper Prism \
          (unlike whitequark) interposes between a call and its arguments.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mode = match options.style("Mode")? {
            "conservative" => Mode::Conservative,
            _ => Mode::Aggressive,
        };
        Ok(Self { mode, plus_spans: HashSet::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.plus_spans.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = plus_call(node) else { return };
        let span = node.span();
        self.plus_spans.insert(span);
        if effective_parent(ctx)
            .is_some_and(|p| p.kind == NodeKind::CallNode && self.plus_spans.contains(&p.span))
        {
            // Not topmost: an ancestor `+` call already saw (or will see,
            // impossible since ancestors are visited first) this span and
            // owns the whole chain's report.
            return;
        }

        let mut parts = Vec::new();
        let mut reportable = false;
        collect_chain(call.as_node(), ctx, &mut parts, &mut reportable);
        if !reportable {
            return;
        }
        if self.mode == Mode::Conservative
            && !matches!(parts.first(), Some(Node::StringNode { .. }))
        {
            return;
        }

        if parts.iter().any(|part| is_uncorrectable(part, ctx)) {
            ctx.report(&Self::META, span, MSG);
        } else {
            let replacement = build_replacement(&parts, ctx);
            let fix = Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, replacement)],
            };
            ctx.report_with_fix(&Self::META, span, MSG, fix);
        }
    }
}

/// The node's effective parent for chain-membership purposes: its immediate
/// ancestor, skipping through an interposed `ArgumentsNode` (Prism wraps a
/// call's arguments in their own node; whitequark's `send` node has no such
/// wrapper, so RuboCop's `node.parent` would see the call directly).
fn effective_parent(ctx: &Context<'_>) -> Option<NodeInfo> {
    let ancestors = ctx.ancestors();
    let mut i = ancestors.len();
    while i > 0 {
        let info = ancestors[i - 1];
        if info.kind == NodeKind::ArgumentsNode {
            i -= 1;
            continue;
        }
        return Some(info);
    }
    None
}

/// RuboCop's `plus_node?`.
fn plus_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    node.as_call_node().filter(|c| c.name().as_slice() == b"+")
}

/// The call's single positional argument, matching the arity the
/// `string_concatenation?` node pattern requires.
fn first_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let args = call.arguments()?;
    let list = args.arguments();
    if list.len() == 1 {
        list.first()
    } else {
        None
    }
}

fn is_string_literal(node: &Node<'_>) -> bool {
    matches!(node, Node::StringNode { .. })
}

/// RuboCop's `string_concatenation?`.
fn qualifies(call: &CallNode<'_>) -> bool {
    let Some(arg) = first_argument(call) else { return false };
    call.receiver().as_ref().is_some_and(is_string_literal) || is_string_literal(&arg)
}

/// RuboCop's `line_end_concatenation?`.
fn line_end_concatenation(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    let Some(arg) = first_argument(call) else { return false };
    if !is_string_literal(&receiver) || !is_string_literal(&arg) {
        return false;
    }
    if ctx.is_single_line(call.as_node().span()) {
        return false;
    }
    let Some(op) = call.message_loc() else { return false };
    let between = ctx.text(Span::new(op.span().end, arg.span().start));
    for &b in between {
        match b {
            b'\n' => return true,
            b' ' | b'\t' | b'\r' => {}
            _ => return false,
        }
    }
    false
}

/// RuboCop's `collect_parts`, extended to also compute (in the same walk)
/// whether any node in the chain satisfies `string_concatenation?` and is
/// not `line_end_concatenation?`-exempt -- RuboCop gets this for free by
/// firing `on_send` once per qualifying node and always climbing to the same
/// topmost node; a single top-down pass must gather it explicitly instead.
fn collect_chain<'pr>(
    node: Node<'pr>,
    ctx: &Context<'_>,
    parts: &mut Vec<Node<'pr>>,
    reportable: &mut bool,
) {
    if let Some(call) = plus_call(&node) {
        if qualifies(&call) && !line_end_concatenation(&call, ctx) {
            *reportable = true;
        }
        if let Some(receiver) = call.receiver() {
            collect_chain(receiver, ctx, parts, reportable);
        }
        if let Some(arg) = first_argument(&call) {
            collect_chain(arg, ctx, parts, reportable);
        }
    } else {
        parts.push(node);
    }
}

/// RuboCop's `uncorrectable?`.
fn is_uncorrectable(part: &Node<'_>, ctx: &Context<'_>) -> bool {
    !ctx.is_single_line(part.span())
        || is_heredoc_part(part, ctx)
        || contains_block_descendant(part)
}

/// RuboCop's `heredoc?`.
fn is_heredoc_part(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let opening_span = match node {
        Node::StringNode { .. } => {
            node.as_string_node().expect("kind matched").opening_loc().map(|l| l.span())
        }
        Node::InterpolatedStringNode { .. } => node
            .as_interpolated_string_node()
            .expect("kind matched")
            .opening_loc()
            .map(|l| l.span()),
        Node::XStringNode { .. } => {
            Some(node.as_x_string_node().expect("kind matched").opening_loc().span())
        }
        Node::InterpolatedXStringNode { .. } => {
            Some(node.as_interpolated_x_string_node().expect("kind matched").opening_loc().span())
        }
        _ => None,
    };
    opening_span.is_some_and(|span| ctx.text(span).starts_with(b"<<"))
}

/// RuboCop's `part.each_descendant(:any_block).any?`: a regular or numbered-
/// parameter block (`{ }`/`do...end`), or a `->` lambda literal (also a
/// `:block` node in whitequark's unified AST).
fn contains_block_descendant(root: &Node<'_>) -> bool {
    struct Finder {
        found: bool,
    }
    impl<'pr> Visitor<'pr> for Finder {
        fn enter(&mut self, node: &Node<'pr>) {
            if matches!(node, Node::BlockNode { .. } | Node::LambdaNode { .. }) {
                self.found = true;
            }
        }
    }
    let mut finder = Finder { found: false };
    ruby_ast::walk(root, &mut finder);
    finder.found
}

/// RuboCop's `replacement` plus `handle_quotes`.
fn build_replacement(parts: &[Node<'_>], ctx: &Context<'_>) -> Vec<u8> {
    let mut rendered: Vec<Vec<u8>> = parts.iter().map(|part| adjust_str(part, ctx)).collect();
    for part in &mut rendered {
        if part.as_slice() == b"\"" {
            *part = b"\\\"".to_vec();
        }
    }
    let mut out = Vec::with_capacity(2 + rendered.iter().map(Vec::len).sum::<usize>());
    out.push(b'"');
    for part in rendered {
        out.extend(part);
    }
    out.push(b'"');
    out
}

/// RuboCop's `adjust_str`. `:begin` (whitequark's parenthesized-expression
/// wrapper, matching Prism's `ParenthesesNode`/`StatementsNode` pair) and
/// `:dstr` both recurse into their children and join the results;
/// RuboCop's `else` (the catch-all for a non-string part, e.g. a bare method
/// call or a ternary) re-embeds the part's own source in a fresh `#{...}`,
/// which for Prism's `EmbeddedStatementsNode`/`EmbeddedVariableNode` -- the
/// interpolation holes inside a `dstr`, unlike whitequark's inlined
/// children -- is already exactly what their own span covers, so they are
/// copied verbatim instead of being wrapped a second time.
fn adjust_str(node: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    match node {
        Node::StringNode { .. } => {
            let s = node.as_string_node().expect("kind matched");
            if is_single_quoted(node, ctx) {
                escape_simple(s.unescaped())
            } else {
                ruby_inspect_body(s.unescaped())
            }
        }
        Node::InterpolatedStringNode { .. } => node
            .as_interpolated_string_node()
            .expect("kind matched")
            .parts()
            .iter()
            .flat_map(|child| adjust_str(&child, ctx))
            .collect(),
        Node::ParenthesesNode { .. } => {
            match node.as_parentheses_node().expect("kind matched").body() {
                Some(body) => adjust_str(&body, ctx),
                None => Vec::new(),
            }
        }
        Node::StatementsNode { .. } => node
            .as_statements_node()
            .expect("kind matched")
            .body()
            .iter()
            .flat_map(|child| adjust_str(&child, ctx))
            .collect(),
        Node::EmbeddedStatementsNode { .. } | Node::EmbeddedVariableNode { .. } => {
            ctx.text(node.span()).to_vec()
        }
        _ => {
            let mut out = Vec::with_capacity(node.span().len() as usize + 3);
            out.push(b'#');
            out.push(b'{');
            out.extend_from_slice(ctx.text(node.span()));
            out.push(b'}');
            out
        }
    }
}

/// RuboCop's `single_quoted?`.
fn is_single_quoted(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    ctx.text(node.span()).first() == Some(&b'\'')
}

/// RuboCop's single-quoted `adjust_str` branch:
/// `value.gsub(/(\\|"|#\{|#@|#\$)/, '\\\&')`.
fn escape_simple(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' || b == b'"' {
            out.push(b'\\');
            out.push(b);
            i += 1;
        } else if b == b'#' && matches!(bytes.get(i + 1), Some(b'{' | b'@' | b'$')) {
            out.push(b'\\');
            out.push(b'#');
            out.push(bytes[i + 1]);
            i += 2;
        } else {
            out.push(b);
            i += 1;
        }
    }
    out
}

/// RuboCop's double-quoted `adjust_str` branch: `value.inspect[1..-2]`.
fn ruby_inspect_body(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'"' => out.extend_from_slice(b"\\\""),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\t' => out.extend_from_slice(b"\\t"),
            b'\r' => out.extend_from_slice(b"\\r"),
            0x00 => out.extend_from_slice(b"\\0"),
            0x07 => out.extend_from_slice(b"\\a"),
            0x08 => out.extend_from_slice(b"\\b"),
            0x0C => out.extend_from_slice(b"\\f"),
            0x0B => out.extend_from_slice(b"\\v"),
            0x1B => out.extend_from_slice(b"\\e"),
            b'#' if matches!(bytes.get(i + 1), Some(b'{' | b'@' | b'$')) => {
                out.push(b'\\');
                out.push(b'#');
                out.push(bytes[i + 1]);
                i += 2;
                continue;
            }
            0x20..=0x7E => out.push(b),
            _ if b < 0x20 || b == 0x7F => {
                out.extend_from_slice(format!("\\x{b:02X}").as_bytes());
            }
            _ => out.push(b),
        }
        i += 1;
    }
    out
}

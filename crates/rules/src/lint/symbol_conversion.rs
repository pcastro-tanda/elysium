//! `Lint/SymbolConversion`, ported from RuboCop's
//! `lib/rubocop/cop/lint/symbol_conversion.rb` plus the `SymbolHelp` mixin
//! it includes.
//!
//! # Three entry points, one shared correction core
//!
//! Upstream hooks `on_send` (restricted to `to_sym`/`intern`), `on_sym`
//! (every bare symbol literal), and `on_hash` (only under `EnforcedStyle:
//! consistent`). This rule mirrors that with [`NodeKind::CallNode`],
//! [`NodeKind::SymbolNode`] and [`NodeKind::HashNode`].
//!
//! # Recovering `hash_key?`/`colon?` without typed parent access
//!
//! `SymbolHelp#hash_key?` is `node.parent&.pair_type? && node ==
//! node.parent.child_nodes.first`, and `correct_hash_key` separately needs
//! `node.parent.colon?` (a hash-rocket `=>` pair's operator is never `:`,
//! a label pair's -- `a: 1`, `'a': 1`, or the shorthand `a:` -- is always
//! `nil`, folding the colon into the key node's own location instead; see
//! `layout/space_after_colon.rs`'s module doc for the same Prism quirk).
//! [`Context::parent`] only exposes a node's kind/span, not its typed
//! fields, so this rule instead subscribes to [`NodeKind::AssocNode`] and,
//! on `enter`, records `(key_span, is_colon)` for every symbol-keyed pair
//! into `self.hash_keys` -- populated before the key's own `SymbolNode` is
//! entered, since a parent is always entered before its children.
//!
//! A label key's own `SymbolNode` location extends one byte past
//! whitequark's `loc.expression` (through the trailing `:`, confirmed via
//! Prism directly: bareword `a:` is `value_loc("a") + closing_loc(":")`,
//! quoted `'b':` is `opening_loc("'") + value_loc("b") +
//! closing_loc("':")` -- the closing quote and colon sharing one
//! location). [`key_display_span`] drops that trailing byte for colon-style
//! keys, recovering whitequark's exact (quotes-included,
//! colon-excluded) `node.source`/offense range.
//!
//! # `ignored_node?` without a cop-wide ignore set
//!
//! `on_hash`'s `EnforcedStyle: consistent` path calls `ignore_node`/
//! `ignored_node?` to stop `on_sym` from redundantly re-processing a key it
//! already handled (RuboCop visits a hash's `pair` children *after* the
//! hash itself, so `on_hash` always runs first). This rule reproduces that
//! with `self.ignored_hash_keys`, a plain list of spans checked at the top
//! of the `SymbolNode` handler.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::fmt::Write as _;

/// RuboCop's `MSG`.
const MSG: (&str, &str) = ("Unnecessary symbol conversion; use `", "` instead.");
/// RuboCop's `MSG_CONSISTENCY`.
const MSG_CONSISTENCY: (&str, &str) =
    ("Symbol hash key should be quoted for consistency; use `", "` instead.");

/// `RuboCop::AST::MethodDispatchNode::OPERATOR_METHODS` (private in
/// `MethodIdentifierPredicates`), needed by [`is_bare_symbol_name`] to
/// match `Symbol#inspect`'s own bare-operator-symbol allowance.
const OPERATOR_METHODS: &[&str] = &[
    "|", "^", "&", "<=>", "==", "===", "=~", ">", ">=", "<", "<=", "<<", ">>", "+", "-", "*", "/",
    "%", "**", "~", "+@", "-@", "!@", "~@", "[]", "[]=", "!", "!=", "!~", "`",
];

/// Which `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Strict,
    Consistent,
}

/// `Symbol#inspect`'s own bare-vs-quoted decision: a plain identifier
/// (optionally suffixed with `?`/`!`/`=`), or one of the known operator
/// method names.
fn is_bare_symbol_name(value: &str) -> bool {
    if OPERATOR_METHODS.contains(&value) {
        return true;
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else { return false };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    let rest = chars.as_str();
    let body = match rest.chars().next_back() {
        Some('?' | '!' | '=') => &rest[..rest.len() - 1],
        _ => rest,
    };
    body.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `String#inspect`'s escaping for the quoted form of a symbol name.
fn escape_dquote(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{1b}' => out.push_str("\\e"),
            '\u{7}' => out.push_str("\\a"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\u{b}' => out.push_str("\\v"),
            '\0' => out.push_str("\\0"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                write!(out, "\\x{:02X}", c as u32).expect("write to String never fails");
            }
            c => out.push(c),
        }
    }
    out
}

/// RuboCop's `Symbol#inspect` (via `node.value.inspect`): the canonical
/// `:name` or `:"escaped"` source form, leading colon included.
fn symbol_inspect(value: &str) -> String {
    if is_bare_symbol_name(value) {
        format!(":{value}")
    } else {
        format!(":\"{}\"", escape_dquote(value))
    }
}

/// RuboCop's `requires_quotes?`: `sym_node.value.inspect.match?(/^:".*?"|=$/)`.
fn requires_quotes(canonical: &str) -> bool {
    canonical.starts_with(":\"") || canonical.ends_with('=')
}

/// RuboCop's `properly_quoted?`.
fn properly_quoted(style: Style, source: &str, value: &str) -> bool {
    if style == Style::Strict {
        let has_quote = source.contains('\'') || source.contains('"');
        if !has_quote || value.ends_with('=') {
            return true;
        }
    }
    if source == value {
        return true;
    }
    let mut transformed = String::with_capacity(source.len());
    for ch in source.chars() {
        match ch {
            '"' => transformed.push_str("\\\""),
            '\'' => transformed.push('"'),
            c => transformed.push(c),
        }
    }
    transformed == value
}

/// A label key's own location extends through the trailing `:` (see the
/// module doc); drop that byte to recover whitequark's `loc.expression`.
fn key_display_span(node: &Node<'_>, is_colon: bool) -> Span {
    let span = node.span();
    if is_colon {
        Span::new(span.start, span.end - 1)
    } else {
        span
    }
}

/// RuboCop's `DstrNode#value`, restricted to `str`/interpolated-string
/// parts: a literal part contributes its own unescaped text, anything else
/// (an embedded `#{...}`) contributes its own raw source.
fn interpolated_value(ctx: &Context<'_>, node: &Node<'_>) -> String {
    match node.kind() {
        NodeKind::StringNode => {
            let s = node.as_string_node().expect("kind matched");
            String::from_utf8_lossy(s.unescaped()).into_owned()
        }
        NodeKind::InterpolatedStringNode => {
            let d = node.as_interpolated_string_node().expect("kind matched");
            d.parts().iter().map(|part| interpolated_value(ctx, &part)).collect()
        }
        _ => String::from_utf8_lossy(ctx.text(node.span())).into_owned(),
    }
}

/// RuboCop's `dstr_correction`.
fn dstr_correction(ctx: &Context<'_>, node: &Node<'_>) -> String {
    let dstr = node.as_interpolated_string_node().expect("kind matched");
    if let Some(opening) = dstr.opening_loc() {
        if ctx.text(opening.span()) == b"\"" {
            let full = ctx.text(node.span());
            let inner = &full[1..full.len() - 1];
            return format!(":\"{}\"", String::from_utf8_lossy(inner));
        }
    }
    format!(":\"{}\"", interpolated_value(ctx, node))
}

/// RuboCop's `symbol_conversion_correction`.
fn symbol_conversion_correction(ctx: &Context<'_>, receiver: &Node<'_>) -> Option<String> {
    match receiver.kind() {
        NodeKind::StringNode => {
            let s = receiver.as_string_node().expect("kind matched");
            Some(symbol_inspect(&String::from_utf8_lossy(s.unescaped())))
        }
        NodeKind::SymbolNode => {
            let s = receiver.as_symbol_node().expect("kind matched");
            Some(symbol_inspect(&String::from_utf8_lossy(s.unescaped())))
        }
        NodeKind::InterpolatedStringNode if !ext::is_heredoc(receiver) => {
            Some(dstr_correction(ctx, receiver))
        }
        _ => None,
    }
}

/// Checks for unnecessary symbol conversions.
#[derive(Debug, Clone)]
pub struct SymbolConversion {
    style: Style,
    /// `(key_span, is_colon)` for every symbol-keyed `AssocNode`
    /// encountered so far -- see the module doc.
    hash_keys: Vec<(Span, bool)>,
    /// Spans already handled by `on_hash`'s consistent-style path.
    ignored_hash_keys: Vec<Span>,
}

impl SymbolConversion {
    /// RuboCop's `register_offense`, the plain (non-hash-key) case.
    fn register(ctx: &mut Context<'_>, span: Span, correction: &str) {
        let message = format!("{}{correction}{}", MSG.0, MSG.1);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, correction.as_bytes().to_vec())],
            },
        );
    }

    /// RuboCop's `on_send`.
    fn check_call(call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if call.is_safe_navigation() {
            return;
        }
        if !matches!(call.name().as_slice(), b"to_sym" | b"intern") {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(correction) = symbol_conversion_correction(ctx, &receiver) else { return };
        Self::register(ctx, call.as_node().span(), &correction);
    }

    /// RuboCop's `correct_hash_key`, shared by the `on_sym` dispatch and
    /// `on_hash`'s "treat like strict" per-key fallback.
    fn correct_hash_key(
        &self,
        ctx: &mut Context<'_>,
        node: &Node<'_>,
        value: &str,
        is_colon: bool,
    ) {
        if !value.chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            return;
        }
        let canonical = symbol_inspect(value);
        let correction =
            if is_colon { canonical.strip_prefix(':').unwrap_or(&canonical) } else { &canonical };
        let span = key_display_span(node, is_colon);
        let source = String::from_utf8_lossy(ctx.text(span)).into_owned();
        if properly_quoted(self.style, &source, correction) {
            return;
        }
        let message_correction =
            if is_colon { format!("{correction}:") } else { correction.to_owned() };
        let message = format!("{}{message_correction}{}", MSG.0, MSG.1);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, correction.as_bytes().to_vec())],
            },
        );
    }

    /// RuboCop's `correct_inconsistent_hash_keys`.
    fn correct_inconsistent_hash_key(
        &mut self,
        ctx: &mut Context<'_>,
        node: &Node<'_>,
        value: &str,
        is_colon: bool,
    ) {
        let canonical = symbol_inspect(value);
        if requires_quotes(&canonical) {
            return;
        }
        let correction = format!("\"{value}\"");
        let span = key_display_span(node, is_colon);
        let source = String::from_utf8_lossy(ctx.text(span)).into_owned();
        if properly_quoted(self.style, &source, &correction) {
            return;
        }
        let message_correction = format!("{correction}:");
        let message = format!("{}{message_correction}{}", MSG_CONSISTENCY.0, MSG_CONSISTENCY.1);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, correction.into_bytes())],
            },
        );
    }

    /// RuboCop's `on_sym`.
    fn check_sym(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.ignored_hash_keys.contains(&node.span()) {
            return;
        }
        let sym = node.as_symbol_node().expect("kind matched");
        let value = String::from_utf8_lossy(sym.unescaped()).into_owned();
        let canonical = symbol_inspect(&value);
        let source = String::from_utf8_lossy(ctx.text(node.span())).into_owned();
        if properly_quoted(self.style, &source, &canonical) {
            return;
        }
        if ctx.parent().is_some_and(|p| p.kind == NodeKind::AliasMethodNode) {
            return;
        }
        if let Some(parent) = ctx.parent() {
            if parent.kind == NodeKind::ArrayNode
                && ctx.text(Span::new(parent.span.start, parent.span.start + 1)) == b"%"
            {
                return;
            }
        }
        match self.hash_keys.iter().find(|(span, _)| *span == node.span()) {
            Some(&(_, is_colon)) => self.correct_hash_key(ctx, node, &value, is_colon),
            None => Self::register(ctx, node.span(), &canonical),
        }
    }

    /// RuboCop's `on_hash`.
    fn check_hash(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.style != Style::Consistent {
            return;
        }
        let hash = node.as_hash_node().expect("kind matched");
        let keys: Vec<(Node<'_>, String, bool)> = hash
            .elements()
            .iter()
            .filter_map(|element| {
                let assoc = element.as_assoc_node()?;
                let key = assoc.key();
                let sym = key.as_symbol_node()?;
                let value = String::from_utf8_lossy(sym.unescaped()).into_owned();
                let is_colon = assoc.operator_loc().is_none();
                Some((key, value, is_colon))
            })
            .collect();
        let any_requires_quotes =
            keys.iter().any(|(_, value, _)| requires_quotes(&symbol_inspect(value)));
        for (key, value, is_colon) in keys {
            self.ignored_hash_keys.push(key.span());
            if any_requires_quotes {
                self.correct_inconsistent_hash_key(ctx, &key, &value, is_colon);
            } else {
                self.correct_hash_key(ctx, &key, &value, is_colon);
            }
        }
    }
}

impl Rule for SymbolConversion {
    const META: RuleMeta = RuleMeta {
        name: "Lint/SymbolConversion",
        department: Department::Lint,
        summary: "Checks for unnecessary symbol conversions.",
        explanation: "\
Checks for uses of literal strings converted to a symbol where a literal \
symbol could be used instead.

There are two possible styles for this cop. `strict` (default) will \
register an offense for any incorrect usage. `consistent` additionally \
requires hashes to use the same style for every symbol key (ie. if any \
symbol key needs to be quoted it requires all keys to be quoted).

```ruby
# bad
'string'.to_sym
:symbol.to_sym
'underscored_string'.to_sym
:'underscored_symbol'
'hyphenated-string'.to_sym
\"string_#{interpolation}\".to_sym

# good
:string
:symbol
:underscored_string
:underscored_symbol
:'hyphenated-string'
:\"string_#{interpolation}\"
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::SymbolNode, NodeKind::HashNode, NodeKind::AssocNode],
        config: &[linter::ConfigOption {
            name: "EnforcedStyle",
            default: linter::ConfigDefault::Str("strict"),
            allowed: &["strict", "consistent"],
            doc: "The preferred style when quoting symbol hash keys.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "consistent" => Style::Consistent,
            _ => Style::Strict,
        };
        Ok(Self { style, hash_keys: Vec::new(), ignored_hash_keys: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                Self::check_call(&call, ctx);
            }
            NodeKind::SymbolNode => self.check_sym(node, ctx),
            NodeKind::HashNode => self.check_hash(node, ctx),
            NodeKind::AssocNode => {
                let assoc = node.as_assoc_node().expect("kind matched");
                if assoc.key().kind() == NodeKind::SymbolNode {
                    self.hash_keys.push((assoc.key().span(), assoc.operator_loc().is_none()));
                }
            }
            _ => {}
        }
    }
}

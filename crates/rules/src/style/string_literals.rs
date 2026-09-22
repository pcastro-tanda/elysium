//! `Style/StringLiterals`, ported from RuboCop's `lib/rubocop/cop/style/string_literals.rb`
//! plus its `StringLiteralsHelp`/`StringHelp` mixins and the shared
//! `StringLiteralCorrector`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// RuboCop's `message` when `style == :single_quotes`.
const MSG_SINGLE: &str =
    "Prefer single-quoted strings when you don't need string interpolation or special symbols.";
/// RuboCop's `message` when `style == :double_quotes`.
const MSG_DOUBLE: &str = "Prefer double-quoted strings unless you need single quotes to avoid \
extra backslashes for escaping.";
/// RuboCop's `MSG_INCONSISTENT`.
const MSG_INCONSISTENT: &str = "Inconsistent quote style.";

/// The configured `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Single,
    Double,
}

fn message(style: Style) -> &'static str {
    match style {
        Style::Single => MSG_SINGLE,
        Style::Double => MSG_DOUBLE,
    }
}

/// Checks if uses of quotes match the configured preference.
#[derive(Debug, Clone)]
pub struct StringLiterals {
    style: Style,
    consistent_quotes_in_multiline: bool,
    /// Depth of `EmbeddedStatementsNode`/`#{...}` nesting currently open.
    embed_depth: u32,
    /// Depth of `InterpolatedStringNode`/`InterpolatedSymbolNode`/
    /// `InterpolatedRegularExpressionNode` nesting currently open. Matches
    /// RuboCop's `inside_interpolation?`: a plain string is inside
    /// interpolation when both this and `embed_depth` are nonzero.
    interp_depth: u32,
    /// Depth of "ignored" continuation subtrees currently open (RuboCop's
    /// `ignore_node`): direct `StringNode`/`InterpolatedStringNode`
    /// descendants of a backslash-continued string already judged as a
    /// whole by [`Self::check_continuation`] are not judged again
    /// individually.
    ignored_depth: u32,
    /// Parallel to the `InterpolatedStringNode` nesting: whether each open
    /// node pushed onto `ignored_depth`, so `leave` pops symmetrically.
    ignored_stack: Vec<bool>,
}

impl Rule for StringLiterals {
    const META: RuleMeta = RuleMeta {
        name: "Style/StringLiterals",
        department: Department::Style,
        summary: "Checks if uses of quotes match the configured preference.",
        explanation: "\
```ruby
# EnforcedStyle: single_quotes (default)

# bad
\"No special symbols\"
\"No string interpolation\"
\"Just text\"

# good
'No special symbols'
'No string interpolation'
'Just text'
\"Wait! What's #{this}!\"
```

```ruby
# EnforcedStyle: double_quotes

# bad
'Just some text'
'No special chars or interpolation'

# good
\"Just some text\"
\"No special chars or interpolation\"
\"Every string in #{project} uses double_quotes\"
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::StringNode,
            NodeKind::InterpolatedStringNode,
            NodeKind::EmbeddedStatementsNode,
            NodeKind::InterpolatedSymbolNode,
            NodeKind::InterpolatedRegularExpressionNode,
        ],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("single_quotes"),
                allowed: &["single_quotes", "double_quotes"],
                doc: "Preferred string literal quote style.",
            },
            ConfigOption {
                name: "ConsistentQuotesInMultiline",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Strings spanning multiple lines using `\\` for continuation must use the \
same type of quotes on each line.",
            },
        ],
        blind_spots: "\
Ports whitequark's parser quirk that a string literal spanning multiple
*physical* lines with real embedded newlines (not a heredoc, not a `\\n`
escape) is lexed as a `dstr` with one unquoted `str` child per line: by
default (`ConsistentQuotesInMultiline: false`) such a literal is never
checked at all, and even with the option enabled it is judged as a whole
(equivalent to running the single-string check over its full source) but
never autocorrected, matching `StringLiteralCorrector`'s `return if
node.dstr_type?`. Backslash-continued string concatenation (`'a' \\\\\\n'b'`)
is likewise only ever reported, never fixed, matching RuboCop. Strings
nested inside `#{...}` interpolation of another string/symbol/regexp are
never judged individually, matching `inside_interpolation?`; this is
tracked with nesting counters rather than true ancestor walks, so it does
not distinguish further by container kind beyond string/symbol/regexp vs.
everything else (RuboCop itself does not either).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = if options.style("EnforcedStyle")? == "double_quotes" {
            Style::Double
        } else {
            Style::Single
        };
        Ok(Self {
            style,
            consistent_quotes_in_multiline: options.bool("ConsistentQuotesInMultiline"),
            embed_depth: 0,
            interp_depth: 0,
            ignored_depth: 0,
            ignored_stack: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.embed_depth = 0;
        self.interp_depth = 0;
        self.ignored_depth = 0;
        self.ignored_stack.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::EmbeddedStatementsNode { .. } => self.embed_depth += 1,
            Node::InterpolatedSymbolNode { .. }
            | Node::InterpolatedRegularExpressionNode { .. } => {
                self.interp_depth += 1;
            }
            Node::InterpolatedStringNode { .. } => {
                self.interp_depth += 1;
                let n = node.as_interpolated_string_node().expect("kind matched");
                if self.ignored_depth == 0 && self.check_continuation(&n, ctx) {
                    self.ignored_depth += 1;
                    self.ignored_stack.push(true);
                } else {
                    self.ignored_stack.push(false);
                }
            }
            Node::StringNode { .. } if self.ignored_depth == 0 => {
                let n = node.as_string_node().expect("kind matched");
                self.check_string(&n, ctx);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node {
            Node::EmbeddedStatementsNode { .. } => self.embed_depth -= 1,
            Node::InterpolatedSymbolNode { .. }
            | Node::InterpolatedRegularExpressionNode { .. } => {
                self.interp_depth -= 1;
            }
            Node::InterpolatedStringNode { .. } => {
                self.interp_depth -= 1;
                if self.ignored_stack.pop() == Some(true) {
                    self.ignored_depth -= 1;
                }
            }
            _ => {}
        }
    }
}

impl StringLiterals {
    /// RuboCop's `inside_interpolation?`: true for a string literal nested
    /// inside a `#{...}` (or `#@ivar`) interpolation of an enclosing
    /// `dstr`/`dsym`/`regexp`.
    fn inside_interpolation(&self) -> bool {
        self.embed_depth > 0 && self.interp_depth > 0
    }

    /// RuboCop's `on_dstr`: handles a backslash-continued string
    /// (concatenated adjacent string literals, Prism's `InterpolatedStringNode`
    /// with no `opening_loc`) when `ConsistentQuotesInMultiline` is set.
    /// Returns whether the whole subtree should be treated as already
    /// judged (RuboCop's `ignore_node`), suppressing individual checks on
    /// its `StringNode`/`InterpolatedStringNode` parts.
    fn check_continuation(
        &self,
        node: &ruby_ast::node::InterpolatedStringNode<'_>,
        ctx: &mut Context<'_>,
    ) -> bool {
        if node.opening_loc().is_some() || !self.consistent_quotes_in_multiline {
            return false;
        }

        let parts: Vec<Node<'_>> = node.parts().iter().collect();
        let mut openings: Vec<Option<Vec<u8>>> = Vec::with_capacity(parts.len());
        for part in &parts {
            let opening = match part {
                Node::StringNode { .. } => {
                    part.as_string_node().expect("kind matched").opening_loc()
                }
                Node::InterpolatedStringNode { .. } => {
                    part.as_interpolated_string_node().expect("kind matched").opening_loc()
                }
                // Not all_string_literals?: something other than a whole
                // string literal is directly concatenated. Never observed
                // from Prism for backslash continuation; leave unjudged.
                _ => return false,
            };
            openings.push(opening.map(|l| ctx.text(l.span()).to_vec()));
        }

        let mut uniq: Vec<Vec<u8>> = Vec::new();
        for opening in openings.into_iter().flatten() {
            if !uniq.contains(&opening) {
                uniq.push(opening);
            }
        }
        let Some(quote) = uniq.first() else { return true };
        let span = node.location().span();

        if uniq.len() > 1 {
            ctx.report(&Self::META, span, MSG_INCONSISTENT);
            return true;
        }

        if quote.as_slice() == b"'" && self.style == Style::Double {
            let all_wrong =
                parts.iter().all(|p| wrong_quotes(ctx.text(p.location().span()), self.style));
            if all_wrong {
                ctx.report(&Self::META, span, message(self.style));
            }
        } else if quote.as_slice() == b"\"" && self.style == Style::Single {
            let accept_double = parts.iter().any(|p| {
                matches!(p, Node::InterpolatedStringNode { .. })
                    || double_quotes_required(ctx.text(p.location().span()))
            });
            if !accept_double {
                ctx.report(&Self::META, span, message(self.style));
            }
        }
        true
    }

    /// RuboCop's `StringHelp#on_str` plus `StringLiteralsHelp#offense?`.
    fn check_string(&self, node: &ruby_ast::node::StringNode<'_>, ctx: &mut Context<'_>) {
        let Some(opening) = node.opening_loc() else { return };
        let opening_text = ctx.text(opening.span());
        if opening_text.starts_with(b"<<") {
            return; // heredoc: RuboCop's `str`/`dstr` nodes never carry a `begin` loc.
        }
        if self.inside_interpolation() {
            return;
        }

        let span = node.location().span();
        let src = ctx.text(span);
        let is_plain_quote = matches!(opening_text, b"'" | b"\"");

        if is_plain_quote && src.contains(&b'\n') {
            // Whitequark lexes a multi-physical-line plain string as a
            // `dstr` of one unquoted `str` per line: `on_dstr` only fires
            // (and `on_str` never does, since the per-line children lack a
            // `begin` loc) when `ConsistentQuotesInMultiline` is set, and
            // its offense -- equivalent to judging the whole span here,
            // since the per-line regex scans are equivalent to scanning
            // their concatenation -- is never auto-corrected.
            if self.consistent_quotes_in_multiline && wrong_quotes(src, self.style) {
                ctx.report(&Self::META, span, message(self.style));
            }
            return;
        }

        if wrong_quotes(src, self.style) {
            ctx.report_with_fix(
                &Self::META,
                span,
                message(self.style),
                build_fix(node, self.style),
            );
        }
    }
}

/// RuboCop's `Util#double_quotes_required?`: true when `src` (raw source
/// text, including its own delimiters) contains a literal `'`, or a
/// backslash whose maximal run has odd length and is not immediately
/// followed by `"` -- i.e. an escape sequence a single-quoted literal
/// cannot represent losslessly.
fn double_quotes_required(src: &[u8]) -> bool {
    if src.contains(&b'\'') {
        return true;
    }
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'\\' {
            let start = i;
            while i < src.len() && src[i] == b'\\' {
                i += 1;
            }
            let run_len = i - start;
            if run_len % 2 == 1 && src.get(i) != Some(&b'"') {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

/// The inline regex in RuboCop's `StringLiteralsHelp#wrong_quotes?`'s
/// `style == :double_quotes` branch: true when `src` (raw single-quoted
/// source, including its own delimiters) must stay single-quoted because it
/// has a raw `"`, a backslash escape other than `\\` or `\'`, or an
/// interpolation marker (`#{`, `#@`, `#$`).
fn must_stay_single_quoted(src: &[u8]) -> bool {
    let mut i = 0;
    while i < src.len() {
        match src[i] {
            b'"' => return true,
            b'\\' => {
                if let Some(&next) = src.get(i + 1) {
                    if next != b'\'' && next != b'\\' {
                        return true;
                    }
                }
            }
            b'#' => {
                if matches!(src.get(i + 1), Some(b'{' | b'@' | b'$')) {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// RuboCop's `StringLiteralsHelp#wrong_quotes?`.
fn wrong_quotes(src: &[u8], style: Style) -> bool {
    if matches!(src.first(), Some(b'%' | b'?')) {
        return false;
    }
    match style {
        Style::Single => !double_quotes_required(src),
        Style::Double => !must_stay_single_quoted(src),
    }
}

/// RuboCop's `Util#to_string_literal` single-quoted branch, specialised for
/// values that can legally trigger this cop's single-quotes offense: since
/// `wrong_quotes?` (style `single_quotes`) requires no raw `'` and only
/// `\\`-paired backslashes in the source, the decoded value can never
/// contain an apostrophe, so the `needs_escaping?` fallback to `.inspect`
/// never triggers here. Doubles every backslash, then folds `\"` sequences
/// (produced when a backslash immediately precedes a literal `"`) back to a
/// bare `"`, since single-quoted strings never need to escape `"`.
fn single_quote_escape(value: &[u8]) -> Vec<u8> {
    let mut doubled = Vec::with_capacity(value.len() * 2);
    for &b in value {
        if b == b'\\' {
            doubled.push(b'\\');
        }
        doubled.push(b);
    }
    let mut out = Vec::with_capacity(doubled.len());
    let mut i = 0;
    while i < doubled.len() {
        if doubled[i] == b'\\' && doubled.get(i + 1) == Some(&b'"') {
            out.push(b'"');
            i += 2;
        } else {
            out.push(doubled[i]);
            i += 1;
        }
    }
    out
}

/// RuboCop's `Util#to_string_literal` double-quoted branch (`str.inspect`),
/// specialised the same way: `wrong_quotes?` (style `double_quotes`)
/// requires no raw `"` and only `\\`/`\'` backslash escapes in the source,
/// so the decoded value never needs anything beyond escaping a literal `\`
/// or `"`.
fn double_quote_escape(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    for &b in value {
        match b {
            b'\\' => {
                out.push(b'\\');
                out.push(b'\\');
            }
            b'"' => {
                out.push(b'\\');
                out.push(b'"');
            }
            _ => out.push(b),
        }
    }
    out
}

/// RuboCop's `StringLiteralCorrector.correct` for a plain (non-`dstr`) `str`
/// node.
fn build_fix(node: &ruby_ast::node::StringNode<'_>, style: Style) -> Fix {
    let value = node.unescaped();
    let mut replacement = Vec::with_capacity(value.len() + 2);
    match style {
        Style::Single => {
            replacement.push(b'\'');
            replacement.extend(single_quote_escape(value));
            replacement.push(b'\'');
        }
        Style::Double => {
            replacement.push(b'"');
            replacement.extend(double_quote_escape(value));
            replacement.push(b'"');
        }
    }
    Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::replace(node.location().span(), replacement)],
    }
}

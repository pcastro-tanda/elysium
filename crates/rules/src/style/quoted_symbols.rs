//! `Style/QuotedSymbols`, ported from RuboCop's
//! `lib/rubocop/cop/style/quoted_symbols.rb`, its
//! `StringLiteralsHelp`/`SymbolHelp` mixins (ported privately), and
//! `Util#double_quotes_required?`/`#to_string_literal`.
//!
//! whitequark's `:sym`/`:dsym` split matters here: a *single-line* quoted
//! symbol (`:'abc'`, `:"abc"`) is always a plain `sym` node, matched by
//! `on_sym`. A quoted symbol whose content spans multiple *physical*
//! source lines (e.g. `:'a\n  bc'` with a literal embedded newline) is
//! parsed as a `dsym` (the same composed-string shape as an interpolated
//! symbol) even with no interpolation at all -- `on_sym` never fires for
//! it, so the cop silently ignores it. Prism has no such split: a
//! multi-line quoted symbol is still one `SymbolNode`. This is ported as
//! an explicit "skip multi-line" guard, matching the *observed* behavior
//! (verified against a real RuboCop 1.91 run) without a `dsym`/`sym` kind
//! to dispatch on.
//!
//! whitequark's hash-colon-key sugar (`{ 'foo-bar': 1 }`, a quoted symbol
//! used as a hash key with colon style) does not include the trailing `:`
//! in the symbol node's own `source`; Prism's `SymbolNode` bundles it into
//! `closing_loc` instead (`closing_loc` reads `"':"` rather than `"'"`).
//! [`effective_span`] strips that trailing byte back off so the rest of
//! the port can treat `node`'s text as whitequark would.
//!
//! The regexes below have no lookaround/backreference support in this
//! engine's `regex` crate, so they are hand-translated into direct byte
//! scans; see each function's doc comment for the derivation.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_SINGLE: &str = "Prefer single-quoted symbols when you don't need string interpolation \
or special symbols.";
const MSG_DOUBLE: &str = "Prefer double-quoted symbols unless you need single quotes to avoid \
extra backslashes for escaping.";

/// Use a consistent style for quoted symbols.
#[derive(Debug, Clone)]
pub struct QuotedSymbols {
    /// Resolved to `"single_quotes"` or `"double_quotes"`; `EnforcedStyle:
    /// same_as_string_literals` is resolved once here against
    /// `Style/StringLiterals`'s own configuration (`config.cop_enabled?`
    /// falling back to `single_quotes` when that cop is disabled).
    style: &'static str,
}

impl Rule for QuotedSymbols {
    const META: RuleMeta = RuleMeta {
        name: "Style/QuotedSymbols",
        department: Department::Style,
        summary: "Use a consistent style for quoted symbols.",
        explanation: "\
By default uses the same configuration as `Style/StringLiterals`; if that
cop is not enabled, the default `EnforcedStyle` is `single_quotes`.

String interpolation is always kept in double quotes.

```ruby
# EnforcedStyle: same_as_string_literals (default) / single_quotes
# bad
:\"abc-def\"

# good
:'abc-def'
:\"#{str}\"
:\"a\\'b\"
```

```ruby
# EnforcedStyle: double_quotes
# bad
:'abc-def'

# good
:\"abc-def\"
:\"#{str}\"
:\"a\\'b\"
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::SymbolNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("same_as_string_literals"),
            allowed: &["same_as_string_literals", "single_quotes", "double_quotes"],
            doc: "Whether to use the same style as `Style/StringLiterals`, always single quotes, \
or always double quotes.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let configured = options.style("EnforcedStyle")?;
        let style = if configured == "same_as_string_literals" {
            let enabled = options
                .peer("Style/StringLiterals", "Enabled")
                .and_then(OptionValue::as_bool)
                .unwrap_or(true);
            if enabled {
                match options
                    .peer("Style/StringLiterals", "EnforcedStyle")
                    .and_then(OptionValue::as_str)
                {
                    Some("double_quotes") => "double_quotes",
                    _ => "single_quotes",
                }
            } else {
                "single_quotes"
            }
        } else if configured == "double_quotes" {
            "double_quotes"
        } else {
            "single_quotes"
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(sym) = node.as_symbol_node() else { return };
        if !ctx.is_single_line(node.span()) {
            return;
        }

        let hash_colon_key = sym.closing_loc().is_some_and(|loc| {
            let text = ctx.text(loc.span());
            text.len() > 1 && text.ends_with(b":")
        });
        let span = node.span();
        let effective_span =
            if hash_colon_key { Span::new(span.start, span.end - 1) } else { span };
        let src = ctx.text(effective_span);
        if !quoted(src) {
            return;
        }

        let wrong = if self.style == "single_quotes" {
            !double_quotes_required(src)
        } else {
            // `wrong_quotes?(node) || invalid_double_quotes?(node.source)`.
            !has_double_quote_feature(src) || !(src.contains(&b'"') || odd_backslash_run(src))
        };
        if !wrong {
            return;
        }

        let message = if self.style == "single_quotes" { MSG_SINGLE } else { MSG_DOUBLE };

        // Strip the quote characters (and, for a non-hash-colon-key
        // symbol, the leading `:`) to get the bare inner text, then
        // requote it per `self.style`.
        let strip_front: usize = if hash_colon_key { 1 } else { 2 };
        let inner = &src[strip_front..src.len() - 1];
        let requoted = correct_quotes(inner, self.style);
        let mut replacement = Vec::with_capacity(requoted.len() + 1);
        if !hash_colon_key {
            replacement.push(b':');
        }
        replacement.extend(requoted);

        ctx.report_with_fix(
            &Self::META,
            effective_span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(effective_span, replacement)],
            },
        );
    }
}

/// `quoted?`: `sym_node.source.match?(/\A:?(['"]).*?\1\z/m)` -- an
/// optional leading `:`, then a quote character, with the *same* quote
/// character at the very end (no backreference support, so checked
/// directly: first and last byte match and are a quote character).
fn quoted(src: &[u8]) -> bool {
    let s = if src.first() == Some(&b':') { &src[1..] } else { src };
    if s.len() < 2 {
        return false;
    }
    let first = s[0];
    first == s[s.len() - 1] && (first == b'\'' || first == b'"')
}

/// `Util#double_quotes_required?`: `/'|(?<! \\) \\{2}* \\ (?![\\"])/x` --
/// true if `src` contains a literal `'`, or a *maximal run of backslashes
/// of odd length* whose following byte (if any) is not `"`. A run of even
/// length can never satisfy the lookahead-guarded alternative (every
/// partial match inside the run lands on another backslash), so only odd
/// runs matter; greedy backtracking within an odd run always lands on the
/// full run (any partial consumption leaves a backslash immediately
/// after), so only the byte right past the run's end needs checking.
fn double_quotes_required(src: &[u8]) -> bool {
    src.contains(&b'\'') || odd_backslash_run(src)
}

/// The shared `(?<! \\) \\{2}* \\ (?![\\"])` backslash-run alternative
/// used by both [`double_quotes_required`] (`Util#double_quotes_required?`)
/// and `invalid_double_quotes?`'s own regex: true if there is a maximal run
/// of consecutive backslashes of *odd* length whose following byte (if
/// any) is not `"`.
fn odd_backslash_run(src: &[u8]) -> bool {
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

/// `/" | \\[^'\\] | \#[@{$]/x`: true if `src` contains a literal `"`, a
/// backslash followed by a byte that is neither `'` nor `\`, or an
/// interpolation marker (`#{`, `#@`, `#$`). This is `wrong_quotes?`'s own
/// `style == :double_quotes` branch (`!has_double_quote_feature`, see the
/// call site); `invalid_double_quotes?` is the differently-shaped
/// `odd_backslash_run`-based regex above, not this one.
fn has_double_quote_feature(src: &[u8]) -> bool {
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
                if matches!(src.get(i + 1), Some(&(b'@' | b'{' | b'$'))) {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// Replaces every `\<quote>` with a bare `<quote>` (left to right,
/// non-overlapping).
fn unescape_quote(src: &[u8], quote: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'\\' && src.get(i + 1) == Some(&quote) {
            out.push(quote);
            i += 2;
        } else {
            out.push(src[i]);
            i += 1;
        }
    }
    out
}

/// `correct_quotes`. By construction every call site is already known
/// "wrong" for `style` (see [`double_quotes_required`]/
/// [`has_double_quote_feature`]), so the only backslash escapes `inner` can
/// contain are `\\` (preserved) and a delimiter escape for the *other*
/// quote style (`\"` when converting to single quotes, `\'` when
/// converting to double quotes) -- upstream's own `.inspect`-based
/// doubling-then-undoubling dance nets out to exactly this, verified
/// against upstream's spec fixtures byte-for-byte.
fn correct_quotes(inner: &[u8], style: &str) -> Vec<u8> {
    if style == "single_quotes" {
        let unescaped = unescape_quote(inner, b'"');
        let mut out = Vec::with_capacity(unescaped.len() + 2);
        out.push(b'\'');
        out.extend(unescaped);
        out.push(b'\'');
        out
    } else {
        // Unescape `\'` to a bare `'` first (unnecessary once
        // double-quoted), then escape any *bare* `"` as `\"`, leaving an
        // already-escaped `\\` pair untouched (re-escaping it would
        // double it).
        let unescaped = unescape_quote(inner, b'\'');
        let mut out = Vec::with_capacity(unescaped.len() + 2);
        out.push(b'"');
        let mut i = 0;
        while i < unescaped.len() {
            if unescaped[i] == b'\\' {
                out.push(b'\\');
                if let Some(&next) = unescaped.get(i + 1) {
                    out.push(next);
                    i += 2;
                } else {
                    i += 1;
                }
            } else if unescaped[i] == b'"' {
                out.push(b'\\');
                out.push(b'"');
                i += 1;
            } else {
                out.push(unescaped[i]);
                i += 1;
            }
        }
        out.push(b'"');
        out
    }
}

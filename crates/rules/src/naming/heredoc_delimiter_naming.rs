//! `Naming/HeredocDelimiterNaming`, ported from RuboCop's
//! `lib/rubocop/cop/naming/heredoc_delimiter_naming.rb` plus the `Heredoc`
//! mixin it includes.
//!
//! # `on_heredoc` dispatch
//!
//! The `Heredoc` mixin defines `on_str` and aliases `on_dstr`/`on_xstr` to
//! it, but never aliases `on_dxstr` -- so an interpolated backtick heredoc
//! (whitequark's `dxstr`) is silently never visited upstream. This port
//! mirrors that literally: it subscribes to [`NodeKind::StringNode`]
//! (`str`), [`NodeKind::InterpolatedStringNode`] (`dstr`) and
//! [`NodeKind::XStringNode`] (`xstr`), but not
//! [`NodeKind::InterpolatedXStringNode`] (`dxstr`).
//!
//! # Delimiter extraction
//!
//! `delimiter_string` matches `OPENING_DELIMITER` against the heredoc node's
//! source and takes its second capture group. For a heredoc, both
//! whitequark's expression range and Prism's node span cover only the
//! opening token (`<<-END`), so the capture is the delimiter word.
//!
//! # `ForbiddenDelimiters`
//!
//! The default is a `!ruby/regexp`, which reaches the options as its
//! literal text (`/.../i`); [`ruby_regexp`] turns that form back into the
//! pattern with its flags. Ruby's `^`/`$` are always line anchors, hence
//! `(?m)`. Plain strings are compiled as-is, as `Regexp.new(string)` does.
//!
//! # Offense range
//!
//! `node.children.empty?` is true only for a `dstr` (or `xstr`) with zero
//! segments -- a heredoc with no content at all -- in which case upstream
//! reports on the whole node instead of the closing delimiter. Prism has no
//! direct equivalent for `xstr` (its non-interpolated `XStringNode` always
//! carries a `content_loc`, never a segment list), so this port only applies
//! that fallback to [`ruby_ast::node::InterpolatedStringNode`] with empty
//! `parts()`; no fixture exercises either case.

use std::sync::LazyLock;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use regex::Regex;
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Use meaningful heredoc delimiters.";

/// RuboCop's `Heredoc::OPENING_DELIMITER`.
static OPENING_DELIMITER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(<<[~-]?)['"`]?([^'"`]+)['"`]?"#).expect("valid regex"));

/// `config/default.yml`'s `ForbiddenDelimiters` default.
const DEFAULT_FORBIDDEN: &str = r"/(^|\s)(EO[A-Z]{1}|END)(\s|$)/i";

/// Compiles a `ForbiddenDelimiters` entry: a `/source/flags` Ruby regexp
/// literal keeps its `i`/`m`/`x` flags (Ruby `m` is Rust `s`); anything else
/// is a plain pattern.
fn ruby_regexp(pattern: &str) -> Option<Regex> {
    let literal = pattern.strip_prefix('/').and_then(|rest| {
        let end = rest.rfind('/')?;
        let flags = &rest[end + 1..];
        flags.chars().all(|c| matches!(c, 'i' | 'm' | 'x')).then_some((&rest[..end], flags))
    });
    let (source, flags) = literal.unwrap_or((pattern, ""));
    let mut prefix = String::from("(?m");
    for flag in flags.chars() {
        prefix.push(if flag == 'm' { 's' } else { flag });
    }
    prefix.push(')');
    Regex::new(&format!("{prefix}{source}")).ok()
}

/// Use descriptive heredoc delimiters.
#[derive(Debug, Clone)]
pub struct HeredocDelimiterNaming {
    /// Compiled `ForbiddenDelimiters`; an entry that fails to compile as a
    /// regex (as `Regexp.new` would raise for upstream) is dropped.
    forbidden: Vec<Regex>,
}

impl HeredocDelimiterNaming {
    /// RuboCop's `delimiter_string`: the heredoc's own delimiter text (see
    /// module docs for the quoted-vs-bare capture quirk), given the
    /// heredoc's full source text.
    fn delimiter_string(source: &[u8]) -> String {
        let source = String::from_utf8_lossy(source);
        OPENING_DELIMITER
            .captures(&source)
            .and_then(|caps| caps.get(2))
            .map_or_else(String::new, |m| m.as_str().to_string())
    }

    /// RuboCop's `meaningful_delimiters?`.
    fn meaningful_delimiters(&self, delimiters: &str) -> bool {
        if !delimiters.chars().any(|c| c.is_alphanumeric() || c == '_') {
            return false;
        }
        !self.forbidden.iter().any(|re| re.is_match(delimiters))
    }

    /// The offense range: RuboCop's `node.children.empty? ? node :
    /// node.loc.heredoc_end`, given the node's own span and (if any) its
    /// closing-delimiter span. `closing` is `None` only for the
    /// `InterpolatedStringNode`-with-no-parts case (see module docs).
    /// whitequark's `heredoc_end` keeps the closing line's indentation but
    /// not its line terminator; Prism's `closing_loc` includes the latter.
    fn offense_span(ctx: &Context<'_>, node_span: Span, closing: Option<Span>) -> Span {
        let Some(closing) = closing else { return node_span };
        let text = ctx.text(closing);
        let trailing = text.iter().rev().take_while(|b| matches!(b, b'\n' | b'\r')).count();
        Span::new(closing.start, closing.end - u32::try_from(trailing).expect("offset exceeds u32"))
    }
}

impl Rule for HeredocDelimiterNaming {
    const META: RuleMeta = RuleMeta {
        name: "Naming/HeredocDelimiterNaming",
        department: Department::Naming,
        summary: "Use descriptive heredoc delimiters.",
        explanation: "Checks that your heredocs are using meaningful delimiters. By default it \
                      disallows `END` and `EO*`, and can be configured through forbidden listing \
                      additional delimiters.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::StringNode, NodeKind::InterpolatedStringNode, NodeKind::XStringNode],
        config: &[linter::ConfigOption {
            name: "ForbiddenDelimiters",
            default: linter::ConfigDefault::StrList(&[DEFAULT_FORBIDDEN]),
            allowed: &[],
            doc: "Heredoc delimiters matching any of these patterns are forbidden.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let forbidden =
            options.str_list("ForbiddenDelimiters").iter().filter_map(|p| ruby_regexp(p)).collect();
        Ok(Self { forbidden })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !ext::is_heredoc(node) {
            return;
        }
        let node_span = node.span();
        let closing = if let Some(string) = node.as_string_node() {
            string.closing_loc().map(|l| l.span())
        } else if let Some(string) = node.as_interpolated_string_node() {
            if string.parts().is_empty() {
                None
            } else {
                Some(string.closing_loc().map_or(node_span, |l| l.span()))
            }
        } else {
            let Some(string) = node.as_x_string_node() else { return };
            Some(string.closing_loc().span())
        };
        // `None` is the "report on the whole node" fallback only for the
        // empty-`dstr` case; a heredoc-shaped node otherwise always has a
        // closing delimiter, so the fallback there is unreachable in
        // practice but kept for parity with upstream's `node.children.empty?`.
        let delimiters = Self::delimiter_string(ctx.text(node_span));
        if self.meaningful_delimiters(&delimiters) {
            return;
        }
        let span = Self::offense_span(ctx, node_span, closing);
        ctx.report(&Self::META, span, MSG);
    }
}

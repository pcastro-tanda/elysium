//! `Style/RedundantStringEscape`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_string_escape.rb`.
//!
//! Whitequark's parser nests a plain-text `str` child inside a `dstr`
//! (interpolated string), a `regexp`/`xstr` (their text runs), or inside a
//! percent-`%w`/`%W` array's elements, each without its own delimiter
//! location, and `on_str`'s helpers recursively walk `node.parent` to find
//! the enclosing delimiter/interpolation rules. Prism gives those same
//! plain-text runs their own [`ruby_ast::node::StringNode`] (with
//! `opening_loc`/`closing_loc` both `None`) as direct children of
//! [`ruby_ast::node::InterpolatedStringNode`],
//! [`ruby_ast::node::InterpolatedXStringNode`],
//! [`ruby_ast::node::InterpolatedRegularExpressionNode`], or
//! [`ruby_ast::node::ArrayNode`]. [`RedundantStringEscape`] tracks a small
//! stack of those enclosing containers (pushed/popped as they are entered
//! and left) so a delimiter-less [`ruby_ast::node::StringNode`] can look up
//! one level instead of walking `parent` pointers.
//!
//! # Blind spot
//!
//! A `%W`/interpolated-array element that is itself a multi-part
//! interpolated string (e.g. `%W[#{a}b]`) only inherits its enclosing
//! array's delimiter/percent-array context when it has no delimiter loc of
//! its own; a plain-text run nested two containers deep below the array
//! (inside that element's own interpolation) is not currently covered -- no
//! fixture exercises it.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::bytes::Regex;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Redundant escape of %s inside string literal.";

/// Whether an enclosing container's interpolation is disabled (RuboCop's
/// `interpolation_not_enabled?`) and/or it is a heredoc (which always
/// blocks `delimiter?` from matching, regardless of disabled-ness).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Enabled,
    Disabled,
    Heredoc,
    HeredocDisabled,
}

impl Mode {
    fn new(is_heredoc: bool, interpolation_disabled: bool) -> Self {
        match (is_heredoc, interpolation_disabled) {
            (false, false) => Self::Enabled,
            (false, true) => Self::Disabled,
            (true, false) => Self::Heredoc,
            (true, true) => Self::HeredocDisabled,
        }
    }

    fn is_heredoc(self) -> bool {
        matches!(self, Self::Heredoc | Self::HeredocDisabled)
    }

    fn interpolation_disabled(self) -> bool {
        matches!(self, Self::Disabled | Self::HeredocDisabled)
    }
}

/// One enclosing container (`dstr`/`xstr`/`regexp`/percent-`%w`/`%W` array)
/// that a delimiter-less `StringNode` plain-text run is a direct child of.
#[derive(Debug, Clone, Copy, Default)]
struct Container {
    /// This container is a `regexp`/`xstr` interpolation: RuboCop's
    /// `node.parent&.type?(:regexp, :xstr)` early return in `on_str`.
    skip: bool,
    mode: Mode,
    /// RuboCop's `percent_array_literal?`: a `%w`/`%W` array.
    percent_array: bool,
    /// `(open, close)` delimiter bytes, e.g. `('"', '"')` or `('[', ']')`.
    delim: Option<(u8, u8)>,
}

/// Checks for redundant escapes in string literals.
#[derive(Debug, Clone, Default)]
pub struct RedundantStringEscape {
    stack: Vec<Container>,
}

impl Rule for RedundantStringEscape {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantStringEscape",
        department: Department::Style,
        summary: "Checks for redundant escapes in string literals.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::StringNode,
            NodeKind::InterpolatedStringNode,
            NodeKind::InterpolatedXStringNode,
            NodeKind::InterpolatedRegularExpressionNode,
            NodeKind::ArrayNode,
        ],
        config: &[],
        blind_spots: "A `%W`/interpolated-array element that is itself a \
            multi-part interpolated string only inherits the array's \
            delimiter/percent-array context one container deep; a \
            plain-text run nested two containers below the array is not \
            covered.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::ArrayNode { .. } => self.stack.push(array_container(node)),
            Node::InterpolatedStringNode { .. } => {
                self.stack.push(self.interpolated_string_container(node));
            }
            Node::InterpolatedXStringNode { .. }
            | Node::InterpolatedRegularExpressionNode { .. } => {
                self.stack.push(Container { skip: true, ..Container::default() });
            }
            Node::StringNode { .. } => self.check_string(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if matches!(
            node,
            Node::ArrayNode { .. }
                | Node::InterpolatedStringNode { .. }
                | Node::InterpolatedXStringNode { .. }
                | Node::InterpolatedRegularExpressionNode { .. }
        ) {
            self.stack.pop();
        }
    }
}

/// RuboCop's `percent_w_literal?`/`percent_w_upper_literal?`/
/// `single_quoted?` applied to an [`ruby_ast::node::ArrayNode`]'s own
/// opening/closing locations, pushed for its direct `StringNode` elements.
fn array_container(node: &Node<'_>) -> Container {
    let a = node.as_array_node().expect("kind matched");
    let opening = a.opening_loc().map(|l| l.as_slice());
    let closing = a.closing_loc().map(|l| l.as_slice());
    let lower = opening.is_some_and(|o| o.starts_with(b"%w"));
    let upper = opening.is_some_and(|o| o.starts_with(b"%W"));
    Container {
        skip: false,
        mode: Mode::new(false, lower),
        percent_array: lower || upper,
        delim: delim_of(opening, closing),
    }
}

/// `(open, close)` delimiter bytes from an opening/closing location pair,
/// e.g. `%q(` / `)` -> `('(', ')')`.
fn delim_of(opening: Option<&[u8]>, closing: Option<&[u8]>) -> Option<(u8, u8)> {
    let o = opening?;
    let c = closing?;
    Some((*o.last()?, *c.first()?))
}

/// RuboCop's `MatchRange`'s `each_match_range` pattern, shared by every
/// call site.
static ESCAPE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\\.").expect("valid regex"));

impl RedundantStringEscape {
    /// RuboCop's `interpolation_not_enabled?`/`delimiter?` applied to an
    /// [`ruby_ast::node::InterpolatedStringNode`] (a `dstr`), pushed for its
    /// direct `StringNode` plain-text runs. Inherits the enclosing
    /// container wholesale when this `dstr` has no delimiter of its own
    /// (e.g. a `%W` array element that is itself interpolated).
    fn interpolated_string_container(&self, node: &Node<'_>) -> Container {
        let n = node.as_interpolated_string_node().expect("kind matched");
        let opening = n.opening_loc().map(|l| l.as_slice());
        let Some(o) = opening else {
            return self.stack.last().copied().unwrap_or_default();
        };
        let closing = n.closing_loc().map(|l| l.as_slice());
        let is_heredoc = ruby_ast::ext::is_heredoc(node);
        let single_quoted = o == b"'";
        let percent_q = o.starts_with(b"%q");
        let heredoc_disabled = is_heredoc && o.ends_with(b"'");
        Container {
            skip: false,
            mode: Mode::new(is_heredoc, single_quoted || percent_q || heredoc_disabled),
            percent_array: false,
            delim: delim_of(Some(o), closing),
        }
    }

    /// RuboCop's `on_str`.
    fn check_string(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let s = node.as_string_node().expect("kind matched");
        let opening = s.opening_loc().map(|l| l.as_slice());
        if opening == Some(b"?") {
            return; // RuboCop's `node.character_literal?`.
        }

        // RuboCop's `node.parent&.type?(:regexp, :xstr)`.
        let own_heredoc; // RuboCop's `node.heredoc?` (only true for a direct, non-nested heredoc).
        let mode: Mode;
        let percent_array;
        let delim;
        let contents: Span;

        if let Some(o) = opening {
            let is_heredoc = ruby_ast::ext::is_heredoc(node);
            own_heredoc = is_heredoc;
            let single_quoted = o == b"'";
            let percent_q = o.starts_with(b"%q");
            let heredoc_disabled = is_heredoc && o.ends_with(b"'");
            mode = Mode::new(is_heredoc, single_quoted || percent_q || heredoc_disabled);
            percent_array = false;
            let closing = s.closing_loc().map(|l| l.as_slice());
            delim = delim_of(Some(o), closing);
            contents = if is_heredoc { s.content_loc().span() } else { node.span() };
        } else {
            let Some(top) = self.stack.last() else { return };
            if top.skip {
                return;
            }
            own_heredoc = false;
            mode = top.mode;
            percent_array = top.percent_array;
            delim = top.delim;
            contents = node.span();
        }

        let src = ctx.source().bytes();
        let body = ctx.text(contents);
        for m in ESCAPE.find_iter(body) {
            let m_start = contents.start + u32::try_from(m.start()).expect("offset fits u32");
            let m_end = contents.start + u32::try_from(m.end()).expect("offset fits u32");
            let Some(escaped) =
                std::str::from_utf8(&m.as_bytes()[1..]).ok().and_then(|s| s.chars().next())
            else {
                continue;
            };

            if mode.interpolation_disabled()
                || escaped.is_alphanumeric()
                || escaped == '\\'
                || escaped == '\n'
                || (escaped == ' ' && (percent_array || own_heredoc))
                || disabling_interpolation(src, m_start, m_end)
                || delimiter_allows(mode.is_heredoc(), delim, escaped)
            {
                continue;
            }

            let span = Span::new(m_start, m_end);
            ctx.report_with_fix(
                &RedundantStringEscape::META,
                span,
                MSG.replacen("%s", &escaped.to_string(), 1),
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::delete(Span::new(m_start, m_start + 1))],
                },
            );
        }
    }
}

/// RuboCop's `delimiter?`, given the already-resolved effective
/// heredoc-ness and delimiter pair.
fn delimiter_allows(effective_heredoc: bool, delim: Option<(u8, u8)>, escaped: char) -> bool {
    if effective_heredoc {
        return false;
    }
    match delim {
        None => true,
        Some((open, close)) => {
            let mut buf = [0_u8; 4];
            let s = escaped.encode_utf8(&mut buf);
            s.len() == 1 && (s.as_bytes()[0] == open || s.as_bytes()[0] == close)
        }
    }
}

/// RuboCop's `disabling_interpolation?`, operating on absolute byte offsets
/// into the full source buffer (RuboCop's `Parser::Source::Range#resize`/
/// `#adjust` read from the underlying buffer, not just the match's
/// enclosing string's contents range).
fn disabling_interpolation(src: &[u8], m_start: u32, m_end: u32) -> bool {
    let len = u32::try_from(src.len()).expect("file fits u32");
    let three_end = (m_start + 3).min(len);
    let three = &src[m_start as usize..three_end as usize];
    if three.starts_with(b"\\#") && three.get(2).is_some_and(|b| matches!(b, b'{' | b'$' | b'@')) {
        return true;
    }

    let five_start = m_start.saturating_sub(2);
    let window = &src[five_start as usize..three_end as usize];
    if window.len() >= 4
        && window[0] != b'\\'
        && window[1] == b'#'
        && window[2] == b'\\'
        && matches!(window[3], b'{' | b'$' | b'@')
    {
        return true;
    }

    let four_end = (m_start + 4).min(len);
    let four = &src[m_start as usize..four_end as usize];
    if four == b"\\#\\{" {
        return true;
    }

    let _ = m_end;
    false
}

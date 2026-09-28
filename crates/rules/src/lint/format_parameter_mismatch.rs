//! `Lint/FormatParameterMismatch`, ported from RuboCop's
//! `lib/rubocop/cop/lint/format_parameter_mismatch.rb` plus the
//! `RuboCop::Cop::Utils::FormatString`/`FormatSequence` helper it relies on
//! (`lib/rubocop/cop/utils/format_string.rb`) to parse a `sprintf`-style
//! format string into its `%`-directives.
//!
//! # What the cop actually needs from `FormatString`
//!
//! `FormatSequence` exposes several fields (`flags`, `width`, `precision`,
//! `arg_number`, `style`, `annotated?`, `template?`, `variable_width?`, ...),
//! but this cop only ever reads four: [`FormatSequence::percent`] (a bare
//! `%%` escape), [`FormatSequence::name`] (a `%<name>s`/`%{name}` reference),
//! [`FormatSequence::arity`] (`@source.scan('*').count + 1` -- a plain
//! substring count, not a parse result) and
//! [`FormatSequence::max_digit_dollar_num`] (`@source.scan(DIGIT_DOLLAR)`,
//! likewise a rescan of the matched text, not a captured value). So this
//! port keeps only enough of the `SEQUENCE` regex to reproduce upstream's
//! match *boundaries* (which is all `arity`/`max_digit_dollar_num` actually
//! depend on) and drops the unused width/precision/flags captures entirely.
//!
//! # Duplicate capture names and the `%#{expr}` ambiguity
//!
//! Ruby's Onigmo engine allows the same named group (`width`, `precision`,
//! `name`, ...) to appear more than once in one pattern under alternation;
//! Rust's `regex` crate rejects this at compile time. Since none of those
//! values are read (see above), every occurrence except the five "does this
//! sequence have a name" sites is rewritten as a non-capturing group, and
//! the surviving `name` sites get distinct group names (`name_a`..`name_e`)
//! merged back into one boolean after matching.
//!
//! Upstream's `TEMPLATE_NAME = /(?<!\#)\{(?<name>\w+)\}/` also carries a
//! negative lookbehind rejecting a `{` immediately after a bare `#`, which
//! Rust's `regex` crate cannot express at all (no lookaround). This is not
//! a cosmetic omission: `FLAG` itself matches a literal `#` (the alternate-
//! form flag, e.g. `%#x`), so without the lookbehind `%#{padding}` on its
//! own (nothing following the interpolation) would misparse as `flags=#`
//! plus a bare `{padding}` template, a sequence upstream never matches.
//! The lookbehind only ever *rejects* a `{` that follows a bare trailing
//! `#` flag with no `WIDTH`/`PRECISION` in between it and the `{` -- both
//! `NUMBER`'s own `\#\{.*?\}` interpolation alternative and `PRECISION`'s
//! leading `.` always leave a non-`#` character right before the `{` -- so
//! it is reproducible without lookaround by splitting the
//! `FLAG* WIDTH? PRECISION? TEMPLATE_NAME` alternative in two: one branch
//! (sharing the ordinary unconstrained `FLAG*` used by the type-branches)
//! requires `WIDTH` and/or `PRECISION` to be present, and a wholly separate
//! top-level branch handles the case where both are absent, constraining
//! its own flag run to not end in a bare `#`
//! (`(?:FLAG)*(?:[ 0+-]|\d+\$)` -- any FLAG run whose *last* token is one
//! of the non-`#` flags, or no flags at all). This reproduces the
//! lookbehind exactly for every `FLAG`/`WIDTH`/`PRECISION` combination the
//! grammar can produce. `%#{padding}s`-style sequences (a type character
//! *follows* the interpolation) still match through the ordinary
//! `FLAG* NUMBER? PRECISION? NAME? TYPE` branch above: once `FLAG*` backs
//! off to zero reps, `NUMBER`'s own `\#\{.*?\}` alternative consumes
//! `#{padding}` directly, exactly as Onigmo's backtracking would --
//! confirmed against upstream directly (`FormatString.new('"%#{padding}s:
//! %s"')` parses to two sequences, `%#{padding}s` and `%s`, matching this
//! cop's own `'does not register an offense when an interpolated width'`
//! fixture). No lookaround is needed anywhere, so the whole pattern lives
//! in the `regex` crate; `fancy-regex` is not a dependency.
//!
//! # A latent upstream crash this port does not reproduce
//!
//! `FormatString#max_digit_dollar_num` is
//! `format_sequences.map(&:max_digit_dollar_num).max` -- over *every*
//! sequence, including percent-escapes, whose own `max_digit_dollar_num` is
//! always `nil`. Ruby's `Array#max` raises `ArgumentError` when asked to
//! compare an `Integer` against a `nil` (confirmed directly: `[1,
//! nil].max` raises `comparison of Integer with nil failed`; `[nil,
//! nil].max` does not, since equal elements never need `<=>`). In every
//! *tested* string this never fires, because a numbered sequence next to a
//! percent-escape's `nil` would make `mixed_formats?` reject the string as
//! invalid first (`Lint::FormatParameterMismatch` never reaches
//! `expected_fields_count` for an invalid string) -- confirmed directly for
//! `'%s %2$s'`, whose `max_digit_dollar_num` really does raise
//! `ArgumentError` when called in isolation, but `valid?` is `false` so the
//! cop's own code path never calls it. A numbered format string that also
//! contains a `%%` escape (e.g. `format('%1$s %%', 'foo')`) is not mixed
//! (only one *non-percent* category exists) yet would still crash real
//! RuboCop on this line, untested by any fixture. [`max_digit_dollar_num`]
//! takes the max over only the `Some` entries instead: behaviour-identical
//! to upstream for every case this cop's spec (or `mixed_formats?`) lets
//! through, and simply does not share the crash.
//!
//! # `heredoc?`
//!
//! Upstream's `heredoc?(node)` is `node.first_argument.source[0, 2] ==
//! '<<'` -- a raw-text prefix check, because whitequark's heredoc node
//! `#source` covers only the opening `<<~TAG` token, not the (later, out of
//! order) body. Prism's heredoc node span covers the *entire* heredoc,
//! body included, so the same prefix trick would not reliably identify one
//! (and would run the `FormatString` parser over the whole multi-line body
//! for `invalid_format_string?`, which upstream's own implementation limit
//! never does). This port uses [`ext::is_heredoc`] (a node-kind check)
//! instead: behaviour-identical to the prefix hack for every non-heredoc
//! node (which is every case this cop's spec exercises, per its own
//! `'heredocs are ignored at the moment'` comment), and correctly
//! recognizes a genuine heredoc where the prefix hack could not.

use std::sync::LazyLock;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use regex::Regex;
use ruby_ast::node::CallNode;
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG_INVALID: &str = "Format string is invalid because formatting sequence types \
                            (numbered, named or unnumbered) are mixed.";

/// `FLAG` from `format_string.rb`, minus its unused `arg_number` capture.
const FLAG: &str = r"(?:[ #0+-]|\d+\$)";
/// `FLAG` minus the bare `#` alternative: any flag token that cannot leave
/// a `#` immediately before what follows it. Used to constrain a flag run
/// that sits directly against a `{name}` template with no `WIDTH`/
/// `PRECISION` in between (see the module doc).
const FLAG_NON_HASH: &str = r"(?:[ 0+-]|\d+\$)";
/// `NUMBER` (covers `WIDTH`/`PRECISION`'s shared grammar), minus captures.
const NUMBER: &str = r"(?:\d+|\*(?:\d+\$)?|\#\{.*?\})";
/// `PRECISION`, minus captures.
const PRECISION: &str = r"(?:\.(?:\d+|\*(?:\d+\$)?|\#\{.*?\})?)";
/// `TYPE`.
const TYPE: &str = "[bBdiouxXeEfgGaAcps]";

/// RuboCop's `FormatString::SEQUENCE`. See the module doc for why this
/// keeps only the `pct`/`name_*` captures and reproduces `TEMPLATE_NAME`'s
/// negative lookbehind without lookaround.
fn sequence_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        let pattern = format!(
            "%(?P<pct>%)\
             |%(?:{FLAG})*(?:(?:{NUMBER}?{PRECISION}?(?:<(?P<name_a>\\w+)>)?\
             |{NUMBER}?(?:<(?P<name_b>\\w+)>){PRECISION}?\
             |(?:<(?P<name_c>\\w+)>)(?:{FLAG})*{NUMBER}?{PRECISION}?){TYPE}\
             |(?:{NUMBER}{PRECISION}?|{PRECISION})\\{{(?P<name_d>\\w+)\\}})\
             |%(?:(?:{FLAG})*{FLAG_NON_HASH})?\\{{(?P<name_e>\\w+)\\}}"
        );
        Regex::new(&pattern).expect("valid format-sequence regex")
    });
    &RE
}

/// `DIGIT_DOLLAR`, rescanned against one already-matched sequence's own
/// text by [`FormatSequence::max_digit_dollar_num`] -- no lookaround
/// needed, so a plain `regex::Regex` suffices here.
fn digit_dollar_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)\$").expect("valid regex"));
    &RE
}

/// RuboCop's `FormatString::FormatSequence`, reduced to the four fields
/// [`crate::lint::format_parameter_mismatch`] actually reads (see the
/// module doc).
struct FormatSequence {
    /// `FormatSequence#percent?`.
    percent: bool,
    /// `FormatSequence#name` (truthiness only; the cop never reads its
    /// value).
    name: bool,
    /// `FormatSequence#arity`.
    arity: u32,
    /// `FormatSequence#max_digit_dollar_num`.
    max_digit_dollar: Option<u32>,
}

/// RuboCop's `FormatString#parse`: scans `text` for every non-overlapping
/// `SEQUENCE` match, in source order.
fn parse_format_sequences(text: &str) -> Vec<FormatSequence> {
    let re = sequence_regex();
    let dd = digit_dollar_regex();
    re.captures_iter(text)
        .map(|caps| {
            let matched = caps.get(0).expect("SEQUENCE always matches").as_str();
            let name = caps.name("name_a").is_some()
                || caps.name("name_b").is_some()
                || caps.name("name_c").is_some()
                || caps.name("name_d").is_some()
                || caps.name("name_e").is_some();
            let percent = caps.name("pct").is_some();
            let arity = u32::try_from(matched.matches('*').count()).unwrap_or(u32::MAX - 1) + 1;
            let max_digit_dollar = dd
                .captures_iter(matched)
                .filter_map(|c| c.get(1))
                .filter_map(|g| g.as_str().parse::<u32>().ok())
                .max();
            FormatSequence { percent, name, arity, max_digit_dollar }
        })
        .collect()
}

/// One category a non-percent [`FormatSequence`] can fall into, for
/// [`is_valid`]'s mixed-format check.
#[derive(PartialEq, Eq)]
enum Category {
    Named,
    Numbered,
    Unnumbered,
}

/// RuboCop's `FormatString#valid?` (`!mixed_formats?`): every non-percent
/// sequence must agree on numbered vs. named vs. unnumbered.
fn is_valid(sequences: &[FormatSequence]) -> bool {
    let mut categories: Vec<Category> = Vec::new();
    for seq in sequences.iter().filter(|s| !s.percent) {
        let category = if seq.name {
            Category::Named
        } else if seq.max_digit_dollar.is_some() {
            Category::Numbered
        } else {
            Category::Unnumbered
        };
        if !categories.contains(&category) {
            categories.push(category);
        }
    }
    categories.len() <= 1
}

/// RuboCop's `expected_fields_count`, given the sequences already parsed
/// from a known string/dstr node's source. See the module doc for why the
/// `max_digit_dollar_num` step drops upstream's all-sequences scan (and its
/// crash) in favour of only the sequences that have one.
fn expected_fields_count(sequences: &[FormatSequence]) -> u32 {
    if sequences.iter().any(|s| s.name) {
        return 1;
    }
    if let Some(max) = sequences.iter().filter_map(|s| s.max_digit_dollar).max() {
        if max != 0 {
            return max;
        }
    }
    sequences.iter().filter(|s| !s.percent).map(|s| s.arity).sum()
}

/// Which of the three shapes `RESTRICT_ON_SEND = %i[format sprintf %]`
/// matched.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Format,
    Sprintf,
    Percent,
}

/// `STRING_TYPES = %i[str dstr]`.
fn is_string_type(node: &Node<'_>) -> bool {
    node.as_string_node().is_some() || node.as_interpolated_string_node().is_some()
}

/// rubocop-ast's `const_type?` shape: `ConstantReadNode`/`ConstantPathNode`.
fn is_const_type(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

/// A constant node's own short name (rubocop-ast's `node.loc.name` text),
/// used only to check it against `Kernel`.
fn const_short_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    if let Some(c) = node.as_constant_read_node() {
        Some(c.name().as_slice())
    } else if let Some(c) = node.as_constant_path_node() {
        c.name().map(|n| n.as_slice())
    } else {
        None
    }
}

/// RuboCop's `format_method?`.
fn is_format_method(name: &[u8], call: &CallNode<'_>) -> bool {
    if let Some(receiver) = call.receiver() {
        if is_const_type(receiver.kind()) && const_short_name(&receiver) != Some(b"Kernel") {
            return false;
        }
    }
    if call.name().as_slice() != name {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let list = args.arguments();
    list.len() > 1 && list.first().is_some_and(|first| is_string_type(&first))
}

/// RuboCop's `percent?`.
fn is_percent(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"%" {
        return false;
    }
    let receiver_is_string = call.receiver().is_some_and(|r| is_string_type(&r));
    let first_arg = call.arguments().and_then(|a| a.arguments().first());
    let first_arg_is_array = first_arg.is_some_and(|n| n.as_array_node().is_some());
    if !receiver_is_string && !first_arg_is_array {
        return false;
    }
    if receiver_is_string && first_arg.is_some_and(|n| ext::is_heredoc(&n)) {
        return false;
    }
    true
}

/// RuboCop's `method_with_format_args?`, resolved to which shape matched.
fn classify(call: &CallNode<'_>) -> Option<Kind> {
    if is_format_method(b"sprintf", call) {
        Some(Kind::Sprintf)
    } else if is_format_method(b"format", call) {
        Some(Kind::Format)
    } else if is_percent(call) {
        Some(Kind::Percent)
    } else {
        None
    }
}

/// RuboCop's `called_on_string?` node-matcher: either a nil/const-receiver
/// call whose first argument is a string/dstr literal (the `format`/
/// `sprintf` shape), or a call directly on a string/dstr literal (the `%`
/// shape).
fn is_called_on_string(call: &CallNode<'_>) -> bool {
    let receiver = call.receiver();
    let receiver_nil_or_const = receiver.is_none_or(|r| is_const_type(r.kind()));
    if receiver_nil_or_const {
        let first_is_string = call
            .arguments()
            .and_then(|a| a.arguments().first())
            .is_some_and(|first| is_string_type(&first));
        if first_is_string {
            return true;
        }
    }
    receiver.is_some_and(|r| is_string_type(&r))
}

/// RuboCop's `splat_args?`.
fn has_splat_argument(call: &CallNode<'_>) -> bool {
    call.arguments()
        .is_some_and(|a| a.arguments().iter().skip(1).any(|n| n.as_splat_node().is_some()))
}

/// RuboCop's `matched_arguments_count?`.
fn matched_arguments_count(expected: i64, passed: i64) -> bool {
    if passed.is_negative() {
        expected < passed.abs()
    } else {
        expected != passed
    }
}

/// RuboCop's `count_matches`/`countable_format?`/`countable_percent?`:
/// `None` is upstream's `:unknown`.
fn num_format_args(
    kind: Kind,
    call: &CallNode<'_>,
    source_node: &Node<'_>,
    first_arg: Option<Node<'_>>,
) -> Option<i64> {
    match kind {
        Kind::Format | Kind::Sprintf => {
            if ext::is_heredoc(source_node) {
                return None;
            }
            let list = call.arguments()?.arguments();
            Some(i64::try_from(list.len()).unwrap_or(i64::MAX) - 1)
        }
        Kind::Percent => {
            let array = first_arg?.as_array_node()?;
            Some(i64::try_from(array.elements().len()).unwrap_or(i64::MAX))
        }
    }
}

/// RuboCop's `on_send`.
fn check(call: &CallNode<'_>, ctx: &mut Context<'_>) {
    let Some(kind) = classify(call) else { return };
    if !is_called_on_string(call) {
        return;
    }

    let selector_span = call.message_loc().map_or_else(|| call.as_node().span(), |l| l.span());

    let first_arg = call.arguments().and_then(|a| a.arguments().first());
    let source_node = match kind {
        Kind::Percent => call.receiver(),
        Kind::Format | Kind::Sprintf => first_arg,
    };
    let Some(source_node) = source_node else { return };

    let source_text = String::from_utf8_lossy(ctx.text(source_node.span()));
    let sequences = parse_format_sequences(&source_text);

    if !is_valid(&sequences) {
        ctx.report(&FormatParameterMismatch::META, selector_span, MSG_INVALID);
        return;
    }

    if kind != Kind::Percent && has_splat_argument(call) {
        return;
    }

    let Some(passed) = num_format_args(kind, call, &source_node, first_arg) else { return };
    let expected = expected_fields_count(&sequences);

    if expected == 0
        && first_arg.is_some_and(|fa| {
            fa.as_interpolated_string_node().is_some() || fa.as_array_node().is_some()
        })
    {
        return;
    }

    if !matched_arguments_count(i64::from(expected), passed) {
        return;
    }

    let method_name = if kind == Kind::Percent {
        "String#%".to_string()
    } else {
        String::from_utf8_lossy(call.name().as_slice()).into_owned()
    };
    let message = format!(
        "Number of arguments ({passed}) to `{method_name}` doesn't match the number of fields \
         ({expected})."
    );
    ctx.report(&FormatParameterMismatch::META, selector_span, message);
}

/// Checks for a mismatch between the number of expected fields for
/// `format`/`sprintf`/`String#%` and what is actually passed as arguments.
#[derive(Debug, Clone)]
pub struct FormatParameterMismatch;

impl Rule for FormatParameterMismatch {
    const META: RuleMeta = RuleMeta {
        name: "Lint/FormatParameterMismatch",
        department: Department::Lint,
        summary: "Checks for a mismatch between the number of expected fields for \
                  format/sprintf/#% and what is actually passed as arguments.",
        explanation: "\
Checks for a mismatch between the number of expected fields for
format/sprintf/#% and what is actually passed as arguments.

In addition, it checks whether different formats are used in the same
format string. Do not mix numbered, unnumbered, and named formats in
the same format string.

```ruby
# bad
format('A value: %s and another: %i', a_value)

# good
format('A value: %s and another: %i', a_value, another)

# bad
format('Unnumbered format: %s and numbered: %2$s', a_value, another)

# good
format('Numbered format: %1$s and numbered %2$s', a_value, another)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
`node.receiver == value_node.receiver`-style structural equality is not needed by this cop, but
two genuine simplifications are: (1) `FormatString#max_digit_dollar_num`'s all-sequences
`Array#max` (which can raise `ArgumentError` in real RuboCop for a numbered format string that
also contains a `%%` escape, untested by any fixture) is replaced by a max over only the
sequences that have a digit-dollar number, never crashing and behaviour-identical for every
string `mixed_formats?` lets through; (2) `heredoc?`'s raw `source[0, 2] == '<<'` prefix check
(meaningful only against whitequark's marker-only heredoc `#source`) is replaced by a heredoc
node-kind check, since Prism's heredoc span covers the full multi-line body -- behaviour-identical
for every non-heredoc case (every case this cop's own spec exercises) and, unlike the prefix hack,
actually recognizes a genuine heredoc format string.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        check(&call, ctx);
    }
}

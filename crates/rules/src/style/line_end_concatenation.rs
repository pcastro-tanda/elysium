//! `Style/LineEndConcatenation`, ported from RuboCop's
//! `lib/rubocop/cop/style/line_end_concatenation.rb`.
//!
//! Upstream walks `processed_source.tokens`, looking at every consecutive
//! (predecessor, operator, successor) token triple. Prism doesn't expose a
//! token stream, so this port instead collects every quoted string literal
//! node (`StringNode`/`InterpolatedStringNode` that has an actual opening
//! delimiter of its own -- this excludes heredocs, whose opening is `<<~`/
//! `<<-` rather than a bare quote, and the literal text segments *inside*
//! an interpolated string, which have no delimiter at all) across the
//! whole file, in source order, and for each adjacent pair scans the raw
//! bytes between them for a bare `+`/`<<` with nothing but whitespace (and
//! an optional `\`-newline continuation) around it -- exactly the
//! adjacency the token stream would see. A pair is skipped outright if the
//! predecessor's span isn't fully before the successor's (i.e. one node
//! nests inside the other, e.g. a literal inside its sibling's own
//! interpolation).
//!
//! One token distinction upstream makes doesn't carry over exactly:
//! `tLBRACK2` (an indexing `[` glued directly onto the previous value) vs.
//! `tLBRACK` (an array-literal/call-argument `[`, which needs no such
//! rejection) differ only by whether whitespace precedes the bracket.
//! Every fixture case that exercises the "high precedence successor"
//! rejection (`**`, `%`, `.`, `[`) has the operator immediately adjacent
//! modulo plain spaces, so this port rejects on the first non-whitespace
//! byte being one of those four, without re-deriving the space-sensitive
//! distinction.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _};
use ruby_source::Span;

/// RuboCop's `MSG`, with `%<operator>s` filled in.
fn message(operator: &str) -> String {
    format!("Use `\\` instead of `{operator}` to concatenate multiline strings.")
}

/// A quoted string literal candidate: its full span (including quotes) and
/// whether it's a standard `'`/`"`-delimited literal (RuboCop's
/// `standard_string_literal?`).
struct Candidate {
    span: Span,
    standard: bool,
}

/// Checks for string literal concatenation at the end of a line.
#[derive(Debug, Clone)]
pub struct LineEndConcatenation;

impl Rule for LineEndConcatenation {
    const META: RuleMeta = RuleMeta {
        name: "Style/LineEndConcatenation",
        department: Department::Style,
        summary: "Use \\ instead of + or << to concatenate two string literals at line end.",
        explanation: "\
Checks for string literal concatenation at the end of a line.

```ruby
# bad
some_str = 'ala' +
           'bala'

some_str = 'ala' <<
           'bala'

# good
some_str = 'ala' \\
           'bala'
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "\
Autocorrection is unsafe because the receiver isn't guaranteed to be a
string: replacing `<<` with `\\` when the receiver is e.g. an array (`array
<< 'foo' <<\\n'bar'`) would produce a syntax error.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let mut candidates = Vec::new();
        let root = ctx.parsed().root();
        let source = ctx.source().bytes();
        collect_candidates(&root, source, &mut candidates);
        candidates.sort_by_key(|c| c.span.start);

        // Upstream pairs *tokens*, so a predecessor's real successor is the
        // very next token after it, regardless of how many string literals
        // are nested inside the predecessor itself (e.g. another string
        // literal inside the predecessor's own interpolation). Skip past
        // any candidate still nested inside `pred` to find that successor.
        for (i, pred) in candidates.iter().enumerate() {
            let Some(succ) = candidates[i + 1..].iter().find(|c| c.span.start >= pred.span.end)
            else {
                continue;
            };
            check_pair(ctx, source, pred, succ);
        }
    }
}

/// Collects every quoted string literal (with its own opening delimiter)
/// under `node`, in source order.
fn collect_candidates(node: &Node<'_>, source: &[u8], out: &mut Vec<Candidate>) {
    ruby_ast::each_descendant(node, &mut |n| match n {
        Node::StringNode { .. } => {
            if let Some(opening) = n.as_string_node().and_then(|sn| sn.opening_loc()) {
                out.push(Candidate {
                    span: n.span(),
                    standard: is_standard_delimiter(source, opening.span()),
                });
            }
        }
        Node::InterpolatedStringNode { .. } => {
            if let Some(opening) = n.as_interpolated_string_node().and_then(|sn| sn.opening_loc()) {
                out.push(Candidate {
                    span: n.span(),
                    standard: is_standard_delimiter(source, opening.span()),
                });
            }
        }
        _ => {}
    });
}

/// RuboCop's `standard_string_literal?`, narrowed to what an opening
/// delimiter can tell us: a bare `'`/`"` (as opposed to a heredoc's `<<~`,
/// or a `%`/`%q`/`%Q`/... percent-literal's multi-character opening).
fn is_standard_delimiter(source: &[u8], opening: Span) -> bool {
    let bytes = &source[opening.start as usize..opening.end as usize];
    matches!(bytes, [b'\'' | b'"'])
}

/// Checks one adjacent (predecessor, successor) string pair for a
/// line-end `+`/`<<` concatenation, mirroring `check_token_set`. The
/// caller has already found `succ` as the first candidate that starts at
/// or after `pred` ends (skipping past anything nested inside `pred`).
fn check_pair(ctx: &mut Context<'_>, source: &[u8], pred: &Candidate, succ: &Candidate) {
    let gap_start = pred.span.end as usize;
    let gap_end = succ.span.start as usize;
    let gap = &source[gap_start..gap_end];

    // The operator must be the first non-whitespace content after the
    // predecessor (a comment here would be a distinct token, breaking
    // adjacency, so it isn't skipped).
    let mut i = 0usize;
    while matches!(gap.get(i), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        i += 1;
    }
    let op_start = i;
    let (op_len, operator) = if gap[i..].starts_with(b"<<") {
        (2usize, "<<")
    } else if gap.get(i) == Some(&b'+') {
        (1usize, "+")
    } else {
        return;
    };
    let op_end = op_start + op_len;

    // Between the operator and the successor, only whitespace and a
    // `\`-newline continuation may appear; anything else (in particular a
    // comment) means the successor isn't the token right after the
    // operator.
    let mut j = op_end;
    loop {
        match gap.get(j) {
            Some(b' ' | b'\t' | b'\r' | b'\n') => j += 1,
            Some(b'\\') if gap.get(j + 1) == Some(&b'\n') => j += 2,
            None => break,
            Some(_) => return,
        }
    }

    if !(pred.standard && succ.standard) {
        return;
    }

    let operator_span = Span::new(
        u32::try_from(gap_start + op_start).expect("offset exceeds u32"),
        u32::try_from(gap_start + op_end).expect("offset exceeds u32"),
    );

    if ctx.same_line(operator_span, succ.span) {
        return;
    }

    // `eligible_next_successor?`: reject when the successor is immediately
    // followed by a higher-precedence operator that would bind to it
    // instead of the concatenation (`**`/`*`, `%`, `.`, `[`).
    let mut k = succ.span.end as usize;
    while matches!(source.get(k), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        k += 1;
    }
    if matches!(source.get(k), Some(b'*' | b'%' | b'.' | b'[')) {
        return;
    }

    let fix_span = extend_fix_span(source, operator_span);
    ctx.report_with_fix(
        &LineEndConcatenation::META,
        operator_span,
        message(operator),
        Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(fix_span, b"\\".to_vec())],
        },
    );
}

/// RuboCop's autocorrect range: the operator, extended right across any
/// trailing spaces/tabs (not newlines), and one more byte if that lands on
/// a pre-existing `\` continuation (so the fix doesn't produce `\\`).
fn extend_fix_span(source: &[u8], operator_span: Span) -> Span {
    let mut end = operator_span.end as usize;
    while matches!(source.get(end), Some(b' ' | b'\t')) {
        end += 1;
    }
    if source.get(end) == Some(&b'\\') {
        end += 1;
    }
    Span::new(operator_span.start, u32::try_from(end).expect("offset exceeds u32"))
}

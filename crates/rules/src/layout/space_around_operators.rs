//! `Layout/SpaceAroundOperators`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_around_operators.rb` plus the
//! `PrecedingFollowingAlignment` and `RationalLiteral` mixins it includes.
//!
//! RuboCop's cop dispatches on whitequark node types (`send`, `lvasgn`,
//! `op_asgn`, `match_pattern`, ...). Prism splits each of those into several
//! concrete node kinds (one per variable kind, one per `=`/`&&=`/`||=`/`op=`
//! flavor), so this rule subscribes to the full family and normalizes each
//! down to `(operator span, right-hand span, is a plain "=" bucket for the
//! alignment heuristic)` before running the shared `check_operator` port.
//!
//! `AllowForAlignment`'s vertical-alignment heuristics
//! (`PrecedingFollowingAlignment`) need per-line indentation/token scans that
//! this engine doesn't expose as a lexer; they are approximated with direct
//! byte scans over `ctx.line_text`, which only ever make the cop *more*
//! lenient on a mismatch (a missed "aligned" detection can suppress a real
//! offense; it can never fabricate one), matching the "false negatives over
//! false positives" project rule. See `blind_spots` below for specifics.

use std::collections::HashSet;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `should_not_have_surrounding_space?`/`offense_message` combined
/// with `PrecedingFollowingAlignment`'s alignment allowances.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct SpaceAroundOperators {
    /// `EnforcedStyleForExponentOperator == 'space'`.
    exponent_space: bool,
    /// `EnforcedStyleForRationalLiterals == 'space'`.
    rational_space: bool,
    /// `AllowForAlignment`.
    allow_for_alignment: bool,
    /// `Layout/HashAlignment`'s `EnforcedHashRocketStyle` includes `table`.
    hash_table_style: bool,
    /// `Layout/ExtraSpacing`'s `ForceEqualSignAlignment`.
    force_equal_sign_alignment: bool,
    /// Lines containing a first `=`/compound-assign (`+=`, ...) token,
    /// RuboCop's memoized `assignment_tokens`/`assignment_lines`. Rebuilt
    /// once per file in [`Rule::file_start`].
    assignment_lines: HashSet<u32>,
    /// `pairs_on_same_line?` for each hash literal currently open, pushed on
    /// entering a `HashNode`/`KeywordHashNode` and popped on leaving it, so
    /// a nested `AssocNode` can read its immediate parent's fact off the top
    /// without re-walking siblings (single-traversal, RuboCop's
    /// `node.parent.pairs_on_same_line?`).
    hash_same_line_stack: Vec<bool>,
}

/// RuboCop's `aligned_with_equals_sign` tri-state (`:yes`/`:no`/`:none`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlignResult {
    Yes,
    No,
    None,
}

/// Ruby's `[ \t]` (RuboCop's `range_with_surrounding_space`/
/// `line_indentation` treat only space and tab as significant whitespace for
/// these checks; embedded `\r` never appears in the fixture set).
fn is_blank(b: u8) -> bool {
    matches!(b, b' ' | b'\t')
}

/// RuboCop's `RangeHelp#final_pos` walking left: one pass over contiguous
/// `[ \t]`, then one pass over contiguous `\n` (blank lines included).
fn extend_left(bytes: &[u8], mut pos: u32) -> u32 {
    while pos > 0 && is_blank(bytes[(pos - 1) as usize]) {
        pos -= 1;
    }
    while pos > 0 && bytes[(pos - 1) as usize] == b'\n' {
        pos -= 1;
    }
    pos
}

/// Mirror of [`extend_left`] walking right.
fn extend_right(bytes: &[u8], mut pos: u32) -> u32 {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    while pos < len && is_blank(bytes[pos as usize]) {
        pos += 1;
    }
    while pos < len && bytes[pos as usize] == b'\n' {
        pos += 1;
    }
    pos
}

/// RuboCop's `ProcessedSource#line_indentation`: length of the line's
/// leading `[ \t]` run (the whole line's length for an all-blank line).
fn line_indentation(ctx: &Context<'_>, line: u32) -> u32 {
    let text = ctx.line_text(line);
    u32::try_from(text.iter().position(|&b| !is_blank(b)).unwrap_or(text.len())).unwrap_or(0)
}

/// RuboCop's `String#blank?` for one source line.
fn is_blank_line(ctx: &Context<'_>, line: u32) -> bool {
    ctx.line_text(line).iter().all(|&b| is_blank(b))
}

/// The character column of the first non-`[ \t]` byte on `line`, or `None`
/// for a blank line (Ruby's `line =~ /\S/`).
fn first_non_ws_col(ctx: &Context<'_>, line: u32) -> Option<u32> {
    let text = ctx.line_text(line);
    text.iter().position(|&b| !is_blank(b)).map(|i| u32::try_from(i).unwrap_or(0))
}

/// A comment that is the first thing on its own line (RuboCop's
/// `aligned_comment_lines`, computed lazily instead of memoized per file
/// since it is only consulted while resolving `AllowForAlignment`).
fn is_comment_starting_line(ctx: &Context<'_>, line: u32) -> bool {
    ctx.comments().iter().any(|c| {
        c.line == line && first_non_ws_col(ctx, line) == Some(ctx.line_col(c.span.start).column)
    })
}

/// Blanks out the contents of `'...'`/`"..."`/`` `...` `` string literals on
/// one line (backslash-escaped characters skipped), so the token scanners
/// below never mistake string content for a real operator token (RuboCop's
/// real lexer never does either). Interpolation, `%`-literals, and
/// multi-line strings are not modeled; a miss there can only ever narrow
/// (never widen) what the `AllowForAlignment` heuristic considers aligned.
fn mask_strings(line: &[u8]) -> Vec<u8> {
    let mut out = line.to_vec();
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < out.len() {
        let b = out[i];
        if let Some(q) = quote {
            if b == b'\\' && i + 1 < out.len() {
                out[i] = b'.';
                out[i + 1] = b'.';
                i += 2;
                continue;
            }
            if b == q {
                quote = None;
            }
            out[i] = b'.';
        } else if matches!(b, b'\'' | b'"' | b'`') {
            quote = Some(b);
            out[i] = b'.';
        }
        i += 1;
    }
    out
}

/// Finds the first token in `line` that is a bare `=` or a compound-assign
/// operator (`+=`, `**=`, `<<=`, `||=`, `&&=`, ...) -- RuboCop-AST's
/// `Token#equal_sign?` (`tEQL`/`tOP_ASGN`; the lexer emits `tOP_ASGN`
/// uniformly for every compound-assignment spelling, including `||=`/`&&=`
/// -- only the parser later distinguishes `op_asgn` from `or_asgn`/
/// `and_asgn` node types), used to build the per-file `assignment_lines`
/// set. Excludes `==`, `===`, `!=`, `<=`, `>=`, `<=>`, `=>`. A best-effort
/// byte scan over a [`mask_strings`]-blanked line (no real lexer): a stray
/// match only ever adds a spurious alignment reference line, which can only
/// make the `AllowForAlignment` heuristic more lenient (false negative, not
/// a false positive).
fn find_assignment_token(line: &[u8]) -> Option<usize> {
    let line = mask_strings(line);
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        if rest.starts_with(b"**=")
            || rest.starts_with(b"<<=")
            || rest.starts_with(b">>=")
            || rest.starts_with(b"||=")
            || rest.starts_with(b"&&=")
        {
            return Some(i);
        }
        if rest.starts_with(b"<=>") {
            i += 3;
            continue;
        }
        if rest.starts_with(b"===") {
            i += 3;
            continue;
        }
        if rest.starts_with(b"==")
            || rest.starts_with(b"!=")
            || rest.starts_with(b"<=")
            || rest.starts_with(b">=")
            || rest.starts_with(b"=>")
        {
            i += 2;
            continue;
        }
        if line[i] == b'='
            || (matches!(line[i], b'+' | b'-' | b'*' | b'/' | b'%' | b'|' | b'&' | b'^' | b'~')
                && i + 1 < line.len()
                && line[i + 1] == b'=')
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Finds the first token in `line` from RuboCop's
/// `ASSIGNMENT_OR_COMPARISON_TOKENS` set (`=`, `==`, `===`, `!=`, `<=`,
/// `>=`, any compound-assign including `||=`/`&&=`, and `<<`), returning its
/// start/end byte offsets and whether it is the literal `<<` (as opposed to
/// one of the `=`-suffixed tokens). Used by `aligned_equals_operator?`, over
/// a [`mask_strings`]-blanked line.
fn find_alignment_token(line: &[u8]) -> Option<(usize, usize, bool)> {
    let line = mask_strings(line);
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        for pat in
            [b"**=".as_slice(), b"<<=", b">>=", b"||=", b"&&=", b"===", b"==", b"!=", b"<=", b">="]
        {
            if rest.starts_with(pat) {
                return Some((i, i + pat.len(), false));
            }
        }
        if rest.starts_with(b"<<") {
            return Some((i, i + 2, true));
        }
        if rest.starts_with(b"<=>") {
            i += 3;
            continue;
        }
        if rest.starts_with(b"=>") {
            i += 2;
            continue;
        }
        if line[i] == b'=' {
            return Some((i, i + 1, false));
        }
        if matches!(line[i], b'+' | b'-' | b'*' | b'/' | b'%' | b'|' | b'&' | b'^' | b'~')
            && i + 1 < line.len()
            && line[i + 1] == b'='
        {
            return Some((i, i + 2, false));
        }
        i += 1;
    }
    None
}

/// RuboCop's `aligned_identical?`: the exact same text sits at the same
/// column on `cand_line`.
fn aligned_identical(ctx: &Context<'_>, span: Span, text: &[u8], cand_line: u32) -> bool {
    let col = ctx.line_col(span.start).column as usize;
    let line = ctx.line_text(cand_line);
    col + text.len() <= line.len() && &line[col..col + text.len()] == text
}

/// RuboCop's `aligned_words?`: either a word boundary (whitespace then
/// non-whitespace) falls at the same column on `cand_line`, or the exact
/// text repeats there.
fn aligned_words(ctx: &Context<'_>, span: Span, text: &[u8], cand_line: u32) -> bool {
    let col = ctx.line_col(span.start).column as usize;
    let line = ctx.line_text(cand_line);
    if col >= 1 && col < line.len() && is_blank(line[col - 1]) && !is_blank(line[col]) {
        return true;
    }
    aligned_identical(ctx, span, text, cand_line)
}

/// RuboCop's `aligned_equals_operator?`.
fn aligned_equals_operator(ctx: &Context<'_>, span: Span, text: &[u8], cand_line: u32) -> bool {
    let Some((_, tok_end, tok_is_lshift)) = find_alignment_token(ctx.line_text(cand_line)) else {
        return false;
    };
    let range_end_col = ctx.line_col(span.end).column as usize;
    if range_end_col != tok_end {
        return false;
    }
    let ends_with_eq = text.last() == Some(&b'=');
    let is_append = text == b"<<";
    ends_with_eq || (is_append && !tok_is_lshift)
}

/// RuboCop's `aligned_operator?`.
fn aligned_operator(ctx: &Context<'_>, span: Span, text: &[u8], cand_line: u32) -> bool {
    aligned_identical(ctx, span, text, cand_line)
        || aligned_equals_operator(ctx, span, text, cand_line)
}

/// RuboCop's `aligned_token?`.
fn aligned_token(ctx: &Context<'_>, span: Span, text: &[u8], cand_line: u32) -> bool {
    aligned_words(ctx, span, text, cand_line) || aligned_equals_operator(ctx, span, text, cand_line)
}

/// RuboCop's `aligned_with_line?` over one candidate direction (nearest
/// non-blank, non-comment-starting line first; skips lines that don't match
/// `indent` when it is given).
fn aligned_with_line(
    ctx: &Context<'_>,
    line_nos: impl Iterator<Item = u32>,
    indent: Option<u32>,
    predicate: &mut impl FnMut(&Context<'_>, u32) -> bool,
) -> bool {
    for lineno in line_nos {
        if is_comment_starting_line(ctx, lineno) {
            continue;
        }
        let Some(idx) = first_non_ws_col(ctx, lineno) else { continue };
        if let Some(ind) = indent {
            if ind != idx {
                continue;
            }
        }
        return predicate(ctx, lineno);
    }
    false
}

/// RuboCop's `aligned_with_adjacent_line?`/`aligned_with_any_line_range?`.
fn aligned_with_adjacent_line(
    ctx: &Context<'_>,
    range_line: u32,
    mut predicate: impl FnMut(&Context<'_>, u32) -> bool,
) -> bool {
    let line_count = ctx.line_count();
    let pre = || (1..range_line).rev();
    let post = || (range_line + 1)..=line_count;
    if aligned_with_line(ctx, pre(), None, &mut predicate) {
        return true;
    }
    if aligned_with_line(ctx, post(), None, &mut predicate) {
        return true;
    }
    if let Some(base_indent) = first_non_ws_col(ctx, range_line) {
        if aligned_with_line(ctx, pre(), Some(base_indent), &mut predicate) {
            return true;
        }
        if aligned_with_line(ctx, post(), Some(base_indent), &mut predicate) {
            return true;
        }
    }
    false
}

/// RuboCop's `aligned_with_operator?`.
fn aligned_with_operator(ctx: &Context<'_>, op_span: Span) -> bool {
    let text = ctx.text(op_span);
    let line = ctx.line_col(op_span.start).line;
    aligned_with_adjacent_line(ctx, line, |ctx, cand| aligned_operator(ctx, op_span, text, cand))
}

/// RuboCop's `aligned_with_something?`.
fn aligned_with_something(ctx: &Context<'_>, right_span: Span) -> bool {
    let text = ctx.text(right_span);
    let line = ctx.line_col(right_span.start).line;
    aligned_with_adjacent_line(ctx, line, |ctx, cand| aligned_token(ctx, right_span, text, cand))
}

/// RuboCop's `relevant_assignment_lines`.
fn relevant_assignment_lines(
    ctx: &Context<'_>,
    assignment_lines: &HashSet<u32>,
    line_range: impl Iterator<Item = u32>,
    first_line: u32,
) -> Vec<u32> {
    let mut result = Vec::new();
    let original_indent = line_indentation(ctx, first_line);
    let mut relevant_at_level = true;
    for line_number in line_range {
        let current_indent = line_indentation(ctx, line_number);
        let blank = is_blank_line(ctx, line_number);
        if (current_indent < original_indent && !blank) || (relevant_at_level && blank) {
            break;
        }
        if assignment_lines.contains(&line_number) && current_indent == original_indent {
            result.push(line_number);
        }
        if !blank {
            relevant_at_level = current_indent == original_indent;
        }
    }
    result
}

/// RuboCop's `IRREGULAR_METHODS`: operator-syntax method names this cop
/// never treats as an operator to check.
fn is_irregular_method(name: &[u8]) -> bool {
    matches!(name, b"[]" | b"!" | b"[]=")
}

/// RuboCop-AST's `MethodIdentifierPredicates::OPERATOR_METHODS`.
fn is_operator_method_name(name: &[u8]) -> bool {
    matches!(
        name,
        b"|" | b"^"
            | b"&"
            | b"<=>"
            | b"=="
            | b"==="
            | b"=~"
            | b">"
            | b">="
            | b"<"
            | b"<="
            | b"<<"
            | b">>"
            | b"+"
            | b"-"
            | b"*"
            | b"/"
            | b"%"
            | b"**"
            | b"~"
            | b"+@"
            | b"-@"
            | b"!@"
            | b"~@"
            | b"[]"
            | b"[]="
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

/// RuboCop-AST's `RationalLiteral#rational_literal?`: `(send (int _) :/
/// (rational _))` -- Ruby parses `2/3r` as a genuine division call, but this
/// cop treats an integer-literal-over-rational-literal division as an
/// indivisible literal and never checks its spacing.
fn is_rational_literal_division(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"/" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    if receiver.as_integer_node().is_none() {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let mut it = args.arguments().iter();
    match (it.next(), it.next()) {
        (Some(arg), None) => arg.as_rational_node().is_some(),
        _ => false,
    }
}

/// RuboCop's `regular_operator?`/`operator_with_regular_syntax?`: a genuine
/// binary-operator call (`a + b`), not a unary use (`-a`), a dotted call
/// (`a.+(b)`), or a `::`-scoped call.
fn is_regular_operator(call: &CallNode<'_>) -> bool {
    let name = call.name().as_slice();
    if !is_operator_method_name(name) || is_irregular_method(name) {
        return false;
    }
    if let Some(op_loc) = call.call_operator_loc() {
        if matches!(op_loc.as_slice(), b"." | b"::") {
            return false;
        }
    }
    if let Some(message_loc) = call.message_loc() {
        // RuboCop-AST's `unary_operation?`: the operator sits at the very
        // start of the call's own source (no receiver text precedes it).
        if message_loc.start_offset() == call.location().start_offset() {
            return false;
        }
    }
    true
}

impl SpaceAroundOperators {
    /// RuboCop's `should_not_have_surrounding_space?`.
    fn should_not_have_surrounding_space(&self, op_text: &[u8], is_rational_rhs: bool) -> bool {
        if op_text == b"**" {
            !self.exponent_space
        } else if op_text == b"/" {
            is_rational_rhs && !self.rational_space
        } else {
            false
        }
    }

    /// RuboCop's `aligned_with_equals_sign`.
    fn aligned_with_equals_sign(
        &self,
        ctx: &Context<'_>,
        op_span: Span,
        op_text: &[u8],
        forward: bool,
    ) -> AlignResult {
        let token_line = ctx.line_col(op_span.start).line;
        let token_indent = line_indentation(ctx, token_line);
        let line_count = ctx.line_count();
        let result = if forward {
            relevant_assignment_lines(
                ctx,
                &self.assignment_lines,
                token_line..=line_count,
                token_line,
            )
        } else {
            relevant_assignment_lines(
                ctx,
                &self.assignment_lines,
                (1..=token_line).rev(),
                token_line,
            )
        };
        let Some(&relevant_line) = result.get(1) else { return AlignResult::None };
        let relevant_indent = line_indentation(ctx, relevant_line);
        if relevant_indent < token_indent {
            return AlignResult::None;
        }
        if aligned_equals_operator(ctx, op_span, op_text, relevant_line) {
            AlignResult::Yes
        } else {
            AlignResult::No
        }
    }

    /// RuboCop's `excess_leading_space?`.
    fn excess_leading_space(
        &self,
        ctx: &Context<'_>,
        op_span: Span,
        leading: &[u8],
        is_plain_assignment: bool,
    ) -> bool {
        if !(self.allow_for_alignment && leading.len() >= 2 && &leading[..2] == b"  ") {
            return false;
        }
        if !is_plain_assignment {
            return !aligned_with_operator(ctx, op_span);
        }
        let op_text = ctx.text(op_span);
        let align_preceding = self.aligned_with_equals_sign(ctx, op_span, op_text, false);
        let align_subsequent = self.aligned_with_equals_sign(ctx, op_span, op_text, true);
        if align_preceding == AlignResult::Yes || align_subsequent == AlignResult::None {
            return false;
        }
        align_subsequent != AlignResult::Yes
    }

    /// RuboCop's `excess_trailing_space?`.
    fn excess_trailing_space(&self, ctx: &Context<'_>, right_span: Span, trailing: &[u8]) -> bool {
        if !(trailing.len() >= 2 && &trailing[trailing.len() - 2..] == b"  ") {
            return false;
        }
        !self.allow_for_alignment || !aligned_with_something(ctx, right_span)
    }

    /// RuboCop's `autocorrect`/`enclose_operator_with_space`.
    fn build_fix(
        &self,
        op_text: &[u8],
        no_space_wanted: bool,
        lead_start: u32,
        trail_end: u32,
        trailing: &[u8],
    ) -> Fix {
        let with_space_span = Span::new(lead_start, trail_end);
        let edits = if no_space_wanted {
            vec![Edit::replace(with_space_span, op_text.to_vec())]
        } else if trailing.contains(&b'\n') {
            let mut repl = Vec::with_capacity(op_text.len() + 2);
            repl.push(b' ');
            repl.extend_from_slice(op_text);
            repl.push(b'\n');
            vec![Edit::replace(with_space_span, repl)]
        } else if self.force_equal_sign_alignment && !trailing.ends_with(b" ") {
            vec![Edit::insert(trail_end, b" ".to_vec())]
        } else {
            let mut repl = Vec::with_capacity(op_text.len() + 2);
            repl.push(b' ');
            repl.extend_from_slice(op_text);
            repl.push(b' ');
            vec![Edit::replace(with_space_span, repl)]
        };
        Fix { applicability: Applicability::Safe, edits }
    }

    /// RuboCop's `check_operator`/`offense_message`/`offense`.
    fn check_operator(
        &self,
        ctx: &mut Context<'_>,
        op_span: Span,
        right_span: Span,
        is_rational_rhs: bool,
        is_plain_assignment: bool,
    ) {
        let bytes = ctx.source().bytes();
        let lead_start = extend_left(bytes, op_span.start);
        let trail_end = extend_right(bytes, op_span.end);
        let leading = &bytes[lead_start as usize..op_span.start as usize];
        let trailing = &bytes[op_span.end as usize..trail_end as usize];

        if leading.first() == Some(&b'\n') {
            return;
        }

        if !trailing.contains(&b'\n') {
            let op_line = ctx.line_col(op_span.start).line;
            if let Some(comment) = ctx.comments().iter().find(|c| c.line == op_line) {
                let comment_col = ctx.line_col(comment.span.start).column;
                let trail_col = ctx.line_col(trail_end).column;
                if trail_col == comment_col {
                    return;
                }
            }
        }

        let op_text = ctx.text(op_span);
        let no_space_wanted = self.should_not_have_surrounding_space(op_text, is_rational_rhs);
        let op_str = String::from_utf8_lossy(op_text);

        let message = if no_space_wanted {
            if leading.is_empty() && trailing.is_empty() {
                return;
            }
            format!("Space around operator `{op_str}` detected.")
        } else if leading.is_empty() || trailing.is_empty() {
            format!("Surrounding space missing for operator `{op_str}`.")
        } else if self.excess_leading_space(ctx, op_span, leading, is_plain_assignment)
            || self.excess_trailing_space(ctx, right_span, trailing)
        {
            format!("Operator `{op_str}` should be surrounded by a single space.")
        } else {
            return;
        };

        let fix = self.build_fix(op_text, no_space_wanted, lead_start, trail_end, trailing);
        ctx.report_with_fix(&Self::META, op_span, message, fix);
    }
}

/// Whether `elements` (a hash literal's pairs) has any two consecutive
/// `AssocNode`s (ignoring `**` splats) starting on the same line (RuboCop's
/// `HashNode#pairs_on_same_line?`).
fn pairs_on_same_line(ctx: &Context<'_>, elements: &ruby_ast::node::NodeList<'_>) -> bool {
    let mut prev_line: Option<u32> = None;
    for el in elements {
        if el.as_assoc_node().is_none() {
            continue;
        }
        let line = ctx.line_col(el.span().start).line;
        if prev_line == Some(line) {
            return true;
        }
        prev_line = Some(line);
    }
    false
}

/// Resolves an `EnforcedStyle`-like own option to `space`/`no_space`,
/// treating an explicit YAML `null` the same as "absent" (unlike
/// `RuleOptions::style`, which currently errors on `Some(OptionValue::Null)`
/// instead of falling back to the schema default -- worked around locally
/// here since every fixture for this cop spells out the full RuboCop
/// `cop_config` shape, including nulled-out keys it doesn't override).
fn resolve_style(
    options: &RuleOptions,
    key: &str,
    default: &'static str,
) -> Result<bool, OptionError> {
    let raw = match options.get(key) {
        Some(OptionValue::Str(s)) => s.as_str(),
        Some(OptionValue::Null) | None => default,
        Some(other) => return Err(options.error(key, format!("expected a string, got {other:?}"))),
    };
    match raw {
        "space" => Ok(true),
        "no_space" => Ok(false),
        other => Err(options.error(
            key,
            format!("unsupported style `{other}`, supported styles are: space, no_space"),
        )),
    }
}

impl Rule for SpaceAroundOperators {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceAroundOperators",
        department: Department::Layout,
        summary: "Checks that operators have space around them, except for ** which should or \
                   shouldn't have surrounding space depending on configuration.",
        explanation: "\
It allows vertical alignment consisting of one or more whitespace around operators.

This cop has `AllowForAlignment` option. When `true`, allows most uses of extra spacing if the
intent is to align with an operator on the previous or next line, not counting empty lines or
comment lines.

```ruby
# bad
total = 3*4
\"apple\"+\"juice\"
my_number = 38/4

# good
total = 3 * 4
\"apple\" + \"juice\"
my_number = 38 / 4
```

`EnforcedStyleForExponentOperator: no_space` (default) requires `a**b`; `space` requires
`a ** b`.

`EnforcedStyleForRationalLiterals: no_space` (default) requires `1/48r`; `space` requires
`1 / 48r`.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::AlternationPatternNode,
            NodeKind::AndNode,
            NodeKind::AssocNode,
            NodeKind::CallAndWriteNode,
            NodeKind::CallNode,
            NodeKind::CallOperatorWriteNode,
            NodeKind::CallOrWriteNode,
            NodeKind::CapturePatternNode,
            NodeKind::ClassNode,
            NodeKind::ClassVariableAndWriteNode,
            NodeKind::ClassVariableOperatorWriteNode,
            NodeKind::ClassVariableOrWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::ConstantAndWriteNode,
            NodeKind::ConstantOperatorWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantPathAndWriteNode,
            NodeKind::ConstantPathOperatorWriteNode,
            NodeKind::ConstantPathOrWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::ConstantWriteNode,
            NodeKind::GlobalVariableAndWriteNode,
            NodeKind::GlobalVariableOperatorWriteNode,
            NodeKind::GlobalVariableOrWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::HashNode,
            NodeKind::IfNode,
            NodeKind::IndexAndWriteNode,
            NodeKind::IndexOperatorWriteNode,
            NodeKind::IndexOrWriteNode,
            NodeKind::InstanceVariableAndWriteNode,
            NodeKind::InstanceVariableOperatorWriteNode,
            NodeKind::InstanceVariableOrWriteNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::KeywordHashNode,
            NodeKind::LocalVariableAndWriteNode,
            NodeKind::LocalVariableOperatorWriteNode,
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::LocalVariableWriteNode,
            NodeKind::MatchRequiredNode,
            NodeKind::MultiWriteNode,
            NodeKind::OrNode,
            NodeKind::RescueNode,
            NodeKind::SingletonClassNode,
        ],
        config: &[
            ConfigOption {
                name: "AllowForAlignment",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allows extra spacing when it lines an operator up with one on an adjacent \
                      line.",
            },
            ConfigOption {
                name: "EnforcedStyleForExponentOperator",
                default: ConfigDefault::Str("no_space"),
                allowed: &["space", "no_space"],
                doc: "Whether `**` should have surrounding space.",
            },
            ConfigOption {
                name: "EnforcedStyleForRationalLiterals",
                default: ConfigDefault::Str("no_space"),
                allowed: &["space", "no_space"],
                doc: "Whether `/` should have surrounding space when dividing by a rational \
                      literal.",
            },
        ],
        blind_spots: "\
`AllowForAlignment`'s adjacent-line heuristics (RuboCop's `PrecedingFollowingAlignment` mixin) are
approximated with raw byte scans over `ctx.line_text` instead of a real token stream: operator/
assignment-token detection (`aligned_equals_operator?`, the per-file `assignment_lines` set feeding
`aligned_with_equals_sign`) does not skip string/comment contents, so a `=`/comparison-like
sequence inside a string literal on a candidate line can be mistaken for a real token. Any such
mismatch only ever grants extra alignment leniency (suppressing a real offense), never fabricates
one. `Layout/HashAlignment`'s `AllowMultipleStyles`/array-of-styles interaction beyond checking
whether `table` appears in `EnforcedHashRocketStyle` is not modeled (RuboCop reads the raw
`Array(...)` the same way). `TargetRubyVersion`-gating of `Layout/SpaceAroundOperators`'s one-line
`=>` pattern-matching check (RuboCop skips it below Ruby 3.0) is not implemented; this rule always
checks it, matching the common case where pattern matching is enabled at all.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let exponent_space =
            resolve_style(options, "EnforcedStyleForExponentOperator", "no_space")?;
        let rational_space =
            resolve_style(options, "EnforcedStyleForRationalLiterals", "no_space")?;
        let allow_for_alignment = options.bool("AllowForAlignment");
        let hash_table_style = match options.peer("Layout/HashAlignment", "EnforcedHashRocketStyle")
        {
            Some(OptionValue::Str(s)) => s == "table",
            Some(OptionValue::List(items)) => items.iter().any(|v| v.as_str() == Some("table")),
            _ => false,
        };
        let force_equal_sign_alignment = options
            .peer("Layout/ExtraSpacing", "ForceEqualSignAlignment")
            .and_then(OptionValue::as_bool)
            .unwrap_or(false);

        Ok(Self {
            exponent_space,
            rational_space,
            allow_for_alignment,
            hash_table_style,
            force_equal_sign_alignment,
            assignment_lines: HashSet::new(),
            hash_same_line_stack: Vec::new(),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.assignment_lines.clear();
        self.hash_same_line_stack.clear();
        for (line_no, span) in ctx.lines() {
            if find_assignment_token(ctx.text(span)).is_some() {
                self.assignment_lines.insert(line_no);
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::HashNode => {
                let h = node.as_hash_node().expect("HashNode kind");
                self.hash_same_line_stack.push(pairs_on_same_line(ctx, &h.elements()));
            }
            NodeKind::KeywordHashNode => {
                let h = node.as_keyword_hash_node().expect("KeywordHashNode kind");
                self.hash_same_line_stack.push(pairs_on_same_line(ctx, &h.elements()));
            }
            NodeKind::SingletonClassNode => {
                let n = node.as_singleton_class_node().expect("SingletonClassNode kind");
                self.check_operator(ctx, n.operator_loc().span(), node.span(), false, false);
            }
            NodeKind::AssocNode => {
                let n = node.as_assoc_node().expect("AssocNode kind");
                if let Some(op_loc) = n.operator_loc() {
                    let same_line = *self.hash_same_line_stack.last().unwrap_or(&false);
                    if !self.hash_table_style || same_line {
                        self.check_operator(ctx, op_loc.span(), node.span(), false, false);
                    }
                }
            }
            NodeKind::IfNode => {
                let n = node.as_if_node().expect("IfNode kind");
                if n.if_keyword_loc().is_none() {
                    if let Some(then_loc) = n.then_keyword_loc() {
                        if let Some(first) = n.statements().and_then(|s| s.body().iter().next()) {
                            self.check_operator(ctx, then_loc.span(), first.span(), false, false);
                        }
                        if let Some(else_node) = n.subsequent().and_then(|s| s.as_else_node()) {
                            if let Some(first) =
                                else_node.statements().and_then(|s| s.body().iter().next())
                            {
                                self.check_operator(
                                    ctx,
                                    else_node.else_keyword_loc().span(),
                                    first.span(),
                                    false,
                                    false,
                                );
                            }
                        }
                    }
                }
            }
            NodeKind::RescueNode => {
                let n = node.as_rescue_node().expect("RescueNode kind");
                if let (Some(op_loc), Some(reference)) = (n.operator_loc(), n.reference()) {
                    self.check_operator(ctx, op_loc.span(), reference.span(), false, false);
                }
            }
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("CallNode kind");
                if is_rational_literal_division(&call) {
                    return;
                }
                if let Some(eq_loc) = call.equal_loc() {
                    if let Some(first) = call.arguments().and_then(|a| a.arguments().iter().next())
                    {
                        self.check_operator(ctx, eq_loc.span(), first.span(), false, false);
                    }
                } else if is_regular_operator(&call) {
                    if let (Some(message_loc), Some(first)) = (
                        call.message_loc(),
                        call.arguments().and_then(|a| a.arguments().iter().next()),
                    ) {
                        let is_rational_rhs =
                            call.name().as_slice() == b"/" && first.as_rational_node().is_some();
                        self.check_operator(
                            ctx,
                            message_loc.span(),
                            first.span(),
                            is_rational_rhs,
                            false,
                        );
                    }
                }
            }
            NodeKind::ClassNode => {
                let n = node.as_class_node().expect("ClassNode kind");
                if let (Some(op_loc), Some(superclass)) =
                    (n.inheritance_operator_loc(), n.superclass())
                {
                    self.check_operator(ctx, op_loc.span(), superclass.span(), false, false);
                }
            }
            NodeKind::OrNode => {
                let n = node.as_or_node().expect("OrNode kind");
                self.check_operator(ctx, n.operator_loc().span(), n.right().span(), false, false);
            }
            NodeKind::AndNode => {
                let n = node.as_and_node().expect("AndNode kind");
                self.check_operator(ctx, n.operator_loc().span(), n.right().span(), false, false);
            }
            NodeKind::MatchRequiredNode => {
                let n = node.as_match_required_node().expect("MatchRequiredNode kind");
                self.check_operator(ctx, n.operator_loc().span(), node.span(), false, false);
            }
            NodeKind::AlternationPatternNode => {
                let n = node.as_alternation_pattern_node().expect("AlternationPatternNode kind");
                self.check_operator(ctx, n.operator_loc().span(), node.span(), false, false);
            }
            NodeKind::CapturePatternNode => {
                let n = node.as_capture_pattern_node().expect("CapturePatternNode kind");
                self.check_operator(ctx, n.operator_loc().span(), node.span(), false, false);
            }
            NodeKind::LocalVariableWriteNode => {
                let w = node.as_local_variable_write_node().expect("LocalVariableWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::LocalVariableAndWriteNode => {
                let w = node
                    .as_local_variable_and_write_node()
                    .expect("LocalVariableAndWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::LocalVariableOrWriteNode => {
                let w =
                    node.as_local_variable_or_write_node().expect("LocalVariableOrWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::LocalVariableOperatorWriteNode => {
                let w = node
                    .as_local_variable_operator_write_node()
                    .expect("LocalVariableOperatorWriteNode kind");
                self.check_operator(
                    ctx,
                    w.binary_operator_loc().span(),
                    w.value().span(),
                    false,
                    false,
                );
            }
            NodeKind::InstanceVariableWriteNode => {
                let w =
                    node.as_instance_variable_write_node().expect("InstanceVariableWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::InstanceVariableAndWriteNode => {
                let w = node
                    .as_instance_variable_and_write_node()
                    .expect("InstanceVariableAndWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::InstanceVariableOrWriteNode => {
                let w = node
                    .as_instance_variable_or_write_node()
                    .expect("InstanceVariableOrWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::InstanceVariableOperatorWriteNode => {
                let w = node
                    .as_instance_variable_operator_write_node()
                    .expect("InstanceVariableOperatorWriteNode kind");
                self.check_operator(
                    ctx,
                    w.binary_operator_loc().span(),
                    w.value().span(),
                    false,
                    false,
                );
            }
            NodeKind::ClassVariableWriteNode => {
                let w = node.as_class_variable_write_node().expect("ClassVariableWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::ClassVariableAndWriteNode => {
                let w = node
                    .as_class_variable_and_write_node()
                    .expect("ClassVariableAndWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::ClassVariableOrWriteNode => {
                let w =
                    node.as_class_variable_or_write_node().expect("ClassVariableOrWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::ClassVariableOperatorWriteNode => {
                let w = node
                    .as_class_variable_operator_write_node()
                    .expect("ClassVariableOperatorWriteNode kind");
                self.check_operator(
                    ctx,
                    w.binary_operator_loc().span(),
                    w.value().span(),
                    false,
                    false,
                );
            }
            NodeKind::GlobalVariableWriteNode => {
                let w = node.as_global_variable_write_node().expect("GlobalVariableWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::GlobalVariableAndWriteNode => {
                let w = node
                    .as_global_variable_and_write_node()
                    .expect("GlobalVariableAndWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::GlobalVariableOrWriteNode => {
                let w = node
                    .as_global_variable_or_write_node()
                    .expect("GlobalVariableOrWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::GlobalVariableOperatorWriteNode => {
                let w = node
                    .as_global_variable_operator_write_node()
                    .expect("GlobalVariableOperatorWriteNode kind");
                self.check_operator(
                    ctx,
                    w.binary_operator_loc().span(),
                    w.value().span(),
                    false,
                    false,
                );
            }
            NodeKind::ConstantWriteNode => {
                let w = node.as_constant_write_node().expect("ConstantWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::ConstantAndWriteNode => {
                let w = node.as_constant_and_write_node().expect("ConstantAndWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::ConstantOrWriteNode => {
                let w = node.as_constant_or_write_node().expect("ConstantOrWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::ConstantOperatorWriteNode => {
                let w =
                    node.as_constant_operator_write_node().expect("ConstantOperatorWriteNode kind");
                self.check_operator(
                    ctx,
                    w.binary_operator_loc().span(),
                    w.value().span(),
                    false,
                    false,
                );
            }
            NodeKind::ConstantPathWriteNode => {
                let w = node.as_constant_path_write_node().expect("ConstantPathWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::ConstantPathAndWriteNode => {
                let w =
                    node.as_constant_path_and_write_node().expect("ConstantPathAndWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::ConstantPathOrWriteNode => {
                let w =
                    node.as_constant_path_or_write_node().expect("ConstantPathOrWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::ConstantPathOperatorWriteNode => {
                let w = node
                    .as_constant_path_operator_write_node()
                    .expect("ConstantPathOperatorWriteNode kind");
                self.check_operator(
                    ctx,
                    w.binary_operator_loc().span(),
                    w.value().span(),
                    false,
                    false,
                );
            }
            NodeKind::MultiWriteNode => {
                let w = node.as_multi_write_node().expect("MultiWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::CallAndWriteNode => {
                let w = node.as_call_and_write_node().expect("CallAndWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::CallOrWriteNode => {
                let w = node.as_call_or_write_node().expect("CallOrWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::CallOperatorWriteNode => {
                let w = node.as_call_operator_write_node().expect("CallOperatorWriteNode kind");
                self.check_operator(
                    ctx,
                    w.binary_operator_loc().span(),
                    w.value().span(),
                    false,
                    false,
                );
            }
            NodeKind::IndexAndWriteNode => {
                let w = node.as_index_and_write_node().expect("IndexAndWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::IndexOrWriteNode => {
                let w = node.as_index_or_write_node().expect("IndexOrWriteNode kind");
                self.check_operator(ctx, w.operator_loc().span(), w.value().span(), false, true);
            }
            NodeKind::IndexOperatorWriteNode => {
                let w = node.as_index_operator_write_node().expect("IndexOperatorWriteNode kind");
                self.check_operator(
                    ctx,
                    w.binary_operator_loc().span(),
                    w.value().span(),
                    false,
                    false,
                );
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if matches!(node.kind(), NodeKind::HashNode | NodeKind::KeywordHashNode) {
            self.hash_same_line_stack.pop();
        }
    }
}

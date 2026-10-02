//! `Style/DocumentDynamicEvalDefinition`, ported from RuboCop's
//! `lib/rubocop/cop/style/document_dynamic_eval_definition.rb`.
//!
//! Upstream reads from `processed_source.lines`/comment tokens directly;
//! this port leans on [`Context::line_text`] and [`Context::comments`] for
//! the same raw text. `arg_node.each_child_node(:begin)` -- whitequark's
//! `begin`-wrapped interpolation inside a `dstr` -- becomes "every
//! `InterpolatedStringNode` part that isn't a `StringNode`" (its literal
//! text runs). `BLOCK_COMMENT_REGEXP`/`COMMENT_REGEXP` (`/^\s*#(?!{)/`,
//! `/\s*#(?!{).*/`) have no direct `regex` crate translation (no
//! lookaround), so they are hand-rolled as byte scans for "a `#` not
//! immediately followed by `{`" in [`has_comment_marker`] and
//! [`strip_trailing_comment_matches`] (the latter replicates `String#gsub`
//! removing every such run, including the run of whitespace immediately
//! before each `#`, through the end of its own line -- `.` does not cross
//! `\n` in Ruby's regex engine, matching `regex`'s default).

use std::collections::BTreeMap;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use regex::Regex;
use ruby_ast::ext::is_heredoc;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Add a comment block showing its appearance if interpolated.";

/// When using `class_eval` (or other `eval`) with string interpolation, add a comment block showing its appearance if interpolated.
#[derive(Debug, Clone)]
pub struct DocumentDynamicEvalDefinition;

impl Rule for DocumentDynamicEvalDefinition {
    const META: RuleMeta = RuleMeta {
        name: "Style/DocumentDynamicEvalDefinition",
        department: Department::Style,
        summary: "When using `class_eval` (or other `eval`) with string interpolation, add a comment block showing its appearance if interpolated.",
        explanation: "When using `class_eval` (or other `eval`) with string interpolation, \
add a comment block showing its appearance if interpolated (a practice used in Rails code).",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        // `RESTRICT_ON_SEND`, never aliased to `on_csend`.
        if call.is_safe_navigation() {
            return;
        }
        if !matches!(
            call.name().as_slice(),
            b"eval" | b"class_eval" | b"module_eval" | b"instance_eval"
        ) {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let Some(arg_node) = args.arguments().first() else { return };
        let Some(dstr) = arg_node.as_interpolated_string_node() else { return };
        let parts: Vec<Node<'_>> = dstr.parts().iter().collect();
        if !parts.iter().any(|part| part.kind() == NodeKind::EmbeddedStatementsNode) {
            return;
        }
        if inline_comment_docs(&parts, ctx) {
            return;
        }
        if is_heredoc(&arg_node) && comment_block_docs(&dstr, &parts, node.span(), ctx) {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };
        ctx.report(&Self::META, message_loc.span(), MSG);
    }
}

/// `inline_comment_docs?`: every interpolation part's own source line
/// carries a trailing (non-interpolation) `#` comment marker.
fn inline_comment_docs(parts: &[Node<'_>], ctx: &Context<'_>) -> bool {
    parts.iter().filter(|part| part.kind() == NodeKind::EmbeddedStatementsNode).all(|part| {
        let line = ctx.line_col(part.span().start).line;
        has_comment_marker(ctx.line_text(line))
    })
}

/// `COMMENT_REGEXP.match?`/`BLOCK_COMMENT_REGEXP`'s shared core test: a `#`
/// not immediately followed by `{` (so a genuine comment marker, not a
/// `#{` interpolation opener).
fn has_comment_marker(line: &[u8]) -> bool {
    comment_marker_position(line, 0).is_some()
}

/// The byte offset within `line` of the first `#` at or after `from` that
/// isn't immediately followed by `{`, or `None`.
fn comment_marker_position(line: &[u8], from: usize) -> Option<usize> {
    (from..line.len()).find(|&i| line[i] == b'#' && line.get(i + 1) != Some(&b'{'))
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

/// `BLOCK_COMMENT_REGEXP` (`/^\s*#(?!{)/`) applied to one physical line:
/// `None` unless (after only leading whitespace) the line starts with a
/// genuine comment marker; `Some` carries the text after that marker.
fn strip_block_comment_prefix(line: &[u8]) -> Option<Vec<u8>> {
    let mut i = 0;
    while i < line.len() && is_ws(line[i]) {
        i += 1;
    }
    if i >= line.len() || line[i] != b'#' || line.get(i + 1) == Some(&b'{') {
        return None;
    }
    Some(line[i + 1..].to_vec())
}

/// `merge_adjacent_comments`: combines a line that matched
/// [`strip_block_comment_prefix`] into the running block keyed by its own
/// line number, joined to the previous line's block (by `\n`) when that
/// block ends on the immediately preceding line.
fn merge_adjacent_comments(map: &mut BTreeMap<u32, Vec<u8>>, line: u32, text: &[u8]) {
    let Some(stripped) = strip_block_comment_prefix(text) else { return };
    if let Some((&last_line, _)) = map.iter().next_back() {
        if last_line == line - 1 {
            let mut merged = map.remove(&last_line).expect("checked above");
            merged.push(b'\n');
            merged.extend_from_slice(&stripped);
            map.insert(line, merged);
            return;
        }
    }
    map.insert(line, stripped);
}

/// `heredoc_comment_blocks`: comment-only lines inside the heredoc body
/// (the lines strictly between the opening `<<~`/`<<-` marker's line and
/// the closing terminator's line), grouped into adjacent blocks.
fn heredoc_comment_blocks(
    dstr: &ruby_ast::node::InterpolatedStringNode<'_>,
    ctx: &Context<'_>,
) -> Vec<Vec<u8>> {
    let (Some(opening), Some(closing)) = (dstr.opening_loc(), dstr.closing_loc()) else {
        return Vec::new();
    };
    let start = ctx.line_col(opening.span().end).line + 1;
    let end = ctx.line_col(closing.span().start).line.saturating_sub(1);
    let mut map = BTreeMap::new();
    if start <= end {
        for line in start..=end {
            merge_adjacent_comments(&mut map, line, ctx.line_text(line));
        }
    }
    map.into_values().collect()
}

/// `preceding_comment_blocks`: real `#` comment tokens whose line falls
/// within the flagged call's own line span, grouped into adjacent blocks.
fn preceding_comment_blocks(call_span: Span, ctx: &Context<'_>) -> Vec<Vec<u8>> {
    let start = ctx.line_col(call_span.start).line;
    let end = ctx.last_line(call_span);
    let mut map = BTreeMap::new();
    for comment in ctx.comments() {
        if comment.kind != linter::CommentKind::Inline {
            continue;
        }
        if comment.line < start || comment.line > end {
            continue;
        }
        merge_adjacent_comments(&mut map, comment.line, ctx.text(comment.span));
    }
    map.into_values().collect()
}

/// `comment_block_docs?`: the heredoc's own body comments plus any real
/// comments around the call, matched against [`comment_regexp`].
fn comment_block_docs(
    dstr: &ruby_ast::node::InterpolatedStringNode<'_>,
    parts: &[Node<'_>],
    call_span: Span,
    ctx: &Context<'_>,
) -> bool {
    let mut comments = heredoc_comment_blocks(dstr, ctx);
    comments.extend(preceding_comment_blocks(call_span, ctx));
    if comments.is_empty() {
        return false;
    }
    let Some(re) = comment_regexp(parts, ctx) else { return false };
    if comments.iter().any(|block| re.is_match(&String::from_utf8_lossy(block))) {
        return true;
    }
    let joined: String =
        comments.iter().map(|block| String::from_utf8_lossy(block).into_owned()).collect();
    re.is_match(&joined)
}

/// `String#gsub(COMMENT_REGEXP, '')`: removes every non-overlapping run of
/// (whitespace, then a genuine `#` comment marker, through the end of its
/// own line) left to right.
fn strip_trailing_comment_matches(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut i = 0usize;
    loop {
        let Some(hash) = comment_marker_position(bytes, i) else {
            out.push_str(&s[i..]);
            break;
        };
        let mut start = hash;
        while start > i && is_ws(bytes[start - 1]) {
            start -= 1;
        }
        out.push_str(&s[i..start]);
        let mut end = hash;
        while end < bytes.len() && bytes[end] != b'\n' {
            end += 1;
        }
        i = end;
    }
    out
}

/// `comment_regexp`: concatenates one regex fragment per `arg_node` child
/// -- `.+` for an interpolation, else [`source_to_regexp`] for a literal
/// text run -- into a single pattern reconstructing the eval'd code's
/// approximate shape.
///
/// Whitequark's parser splits a heredoc's literal text into one `str`
/// child *per physical line* (each including its own trailing `\n`, except
/// a final partial line before an interpolation); Prism instead merges a
/// run of plain text between interpolations into a single multi-line
/// `StringNode`. [`split_whitequark_lines`] re-splits it so each physical
/// line gets its own independent leading `\s*` in the built pattern --
/// without this, a heredoc body line's exact indentation would need to
/// match a doc comment's post-`#`-stripped indentation byte-for-byte.
fn comment_regexp(parts: &[Node<'_>], ctx: &Context<'_>) -> Option<Regex> {
    let mut pattern = String::new();
    for part in parts {
        if part.kind() != NodeKind::StringNode {
            pattern.push_str("(?:.+)");
            continue;
        }
        let s = part.as_string_node().expect("kind matched");
        let raw = String::from_utf8_lossy(ctx.text(s.content_loc().span())).into_owned();
        for line in split_whitequark_lines(&raw) {
            if let Some(fragment) = source_to_regexp(&line) {
                pattern.push_str(&fragment);
            }
        }
    }
    Regex::new(&pattern).ok()
}

/// Splits one Prism `StringNode`'s literal text run into whitequark's
/// per-physical-line `str` children: every complete line keeps its
/// trailing `\n`; a final partial line (no trailing `\n` in the source
/// text, because an interpolation follows immediately) keeps none.
fn split_whitequark_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let ends_with_nl = text.ends_with('\n');
    let mut pieces: Vec<&str> = text.split('\n').collect();
    if ends_with_nl {
        pieces.pop();
    }
    let n = pieces.len();
    pieces
        .into_iter()
        .enumerate()
        .map(|(i, p)| if i + 1 < n || ends_with_nl { format!("{p}\n") } else { p.to_string() })
        .collect()
}

/// `source_to_regexp`: `/\s+/` for a blank fragment; otherwise strips any
/// trailing comment the fragment itself carries, then rebuilds it as a
/// regex that also matches the heredoc-escaped form (`\#` standing in for
/// `#`) of each piece.
fn source_to_regexp(source: &str) -> Option<String> {
    if source.trim().is_empty() {
        return Some(r"(?:\s+)".to_string());
    }
    let stripped = strip_trailing_comment_matches(source);
    let trimmed = stripped.trim();
    if trimmed.is_empty() {
        return None;
    }
    let segments: Vec<String> = trimmed.split("\\#").map(regex::escape).collect();
    Some(format!(r"(?:\s*{})", segments.join(r"\\?#")))
}

//! `Style/FileWrite`, ported from RuboCop's
//! `lib/rubocop/cop/style/file_write.rb`.
//!
//! See `style/file_read.rs`'s module doc for why the block/block-pass and
//! chained-`.write` shapes are matched from two different node
//! perspectives rather than via a `node.parent` lookup.
//!
//! Upstream's `removed_heredocs`/`heredoc_range` restore a heredoc's body
//! when the corrected range would otherwise swallow it (`write_node`'s own
//! span extending past the heredoc, as it does for the block forms, whose
//! own span runs all the way to `end`/`}`). A Prism heredoc `StringNode`'s
//! own span, like whitequark's, already excludes its body (confirmed
//! empirically: `opening_loc`/`closing_loc` bound it to its own line), so
//! `filename`/`content`'s own verbatim text never needs excising. Prism
//! exposes no dedicated `heredoc_body`/`heredoc_end` location either, so
//! [`body_start`] recovers the body's start by scanning for the opening
//! tag's own line break; stacked heredocs sharing one opening line all
//! resolve to that same naive boundary, so the actual per-chunk loop in
//! [`FileWrite::report`] clamps each one forward past the previous heredoc's
//! own close instead. Prism's `closing_loc` also includes the terminator's
//! trailing newline (unlike whitequark's `heredoc_end`), which is trimmed
//! once off the assembled replacement's own end to avoid doubling up with
//! the untouched source byte immediately following `write_node_span`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{
    ext::const_name,
    node::{BlockNode, CallNode},
    LocationExt as _, Node, NodeExt as _, NodeKind,
};
use ruby_source::Span;

/// Upstream's `TRUNCATING_WRITE_MODES`.
const TRUNCATING_MODES: &[&[u8]] = &[b"w", b"wt", b"wb", b"w+", b"w+t", b"w+b"];

/// The write method `File.open(..., mode)` resolves to; see `file_read.rs`'s
/// `Mode` for why this is computed eagerly rather than keeping the borrowed
/// mode bytes around.
#[derive(Debug, Clone, Copy)]
enum Mode {
    Write,
    Binwrite,
}

impl Mode {
    const fn method(self) -> &'static str {
        match self {
            Self::Binwrite => "binwrite",
            Self::Write => "write",
        }
    }
}

/// Upstream's `file_open?`: a plain (non-safe-navigation) `File.open(filename,
/// mode)` call, `mode` being one of [`TRUNCATING_MODES`] (mandatory, unlike
/// `Style/FileRead`'s optional read mode). `call`'s own `block()` must be
/// absent, or the specific `&:write` block-pass -- a *literal* block is
/// invisible to this check either way, matching whitequark wrapping a block
/// call externally (the inner `send` node upstream's pattern actually
/// inspects never carries one).
fn file_open_match<'pr>(call: &CallNode<'pr>) -> Option<(Node<'pr>, Mode)> {
    if call.is_safe_navigation() || call.name().as_slice() != b"open" {
        return None;
    }
    let receiver = call.receiver()?;
    if const_name(&receiver).as_deref() != Some("File") {
        return None;
    }
    let args: Vec<Node<'pr>> =
        call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
    let [filename, mode] = args.as_slice() else { return None };
    let mode_string = mode.as_string_node()?;
    let bytes = mode_string.unescaped();
    if !TRUNCATING_MODES.contains(&bytes) {
        return None;
    }
    let block_ok = match call.block() {
        None => true,
        Some(b) => match b.as_block_argument_node() {
            Some(ba) => ba
                .expression()
                .and_then(|e| e.as_symbol_node())
                .is_some_and(|s| s.unescaped() == b"write"),
            None => true,
        },
    };
    if !block_ok {
        return None;
    }
    let m = if bytes.ends_with(b"b") { Mode::Binwrite } else { Mode::Write };
    Some((*filename, m))
}

/// Upstream's `send_write?`: a plain, single-argument, non-splat `.write`
/// call. Returns the content argument.
fn send_write_content<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    if call.is_safe_navigation() || call.name().as_slice() != b"write" {
        return None;
    }
    let args: Vec<Node<'pr>> = call.arguments()?.arguments().iter().collect();
    let [content] = args.as_slice() else { return None };
    (content.as_splat_node().is_none()).then_some(*content)
}

/// Upstream's `block_write?` (pre-filtered by the caller's
/// `block_arg == lvar` check): a block with exactly one declared parameter,
/// whose sole statement is a single-argument, non-splat `.write` call on
/// that same parameter. Returns the content argument.
fn block_write_content<'pr>(block: &BlockNode<'pr>) -> Option<Node<'pr>> {
    let params = block.parameters().and_then(|p| p.as_block_parameters_node())?;
    if !params.locals().is_empty() {
        return None;
    }
    let params = params.parameters()?;
    if !params.optionals().is_empty()
        || params.rest().is_some()
        || !params.posts().is_empty()
        || !params.keywords().is_empty()
        || params.keyword_rest().is_some()
        || params.block().is_some()
    {
        return None;
    }
    let requireds: Vec<Node<'pr>> = params.requireds().iter().collect();
    let [required] = requireds.as_slice() else { return None };
    let param_name = required.as_required_parameter_node()?.name();

    let stmts = block.body().and_then(|b| b.as_statements_node())?;
    let body: Vec<Node<'pr>> = stmts.body().iter().collect();
    let [stmt] = body.as_slice() else { return None };
    let write_call = stmt.as_call_node()?;
    let content = send_write_content(&write_call)?;
    let receiver_name =
        write_call.receiver()?.as_local_variable_read_node().map(|lvar| lvar.name())?;
    (receiver_name.as_slice() == param_name.as_slice()).then_some(content)
}

/// Every heredoc string literal in `node` itself or any descendant
/// (upstream's `find_heredocs`), as `(opening_loc, closing_loc)` spans.
fn collect_heredocs(ctx: &Context<'_>, node: &Node<'_>, out: &mut Vec<(Span, Span)>) {
    let mut check = |n: &Node<'_>| {
        let opening_closing = match n.kind() {
            NodeKind::StringNode => {
                n.as_string_node().and_then(|s| s.opening_loc().zip(s.closing_loc()))
            }
            NodeKind::InterpolatedStringNode => {
                n.as_interpolated_string_node().and_then(|s| s.opening_loc().zip(s.closing_loc()))
            }
            NodeKind::XStringNode => {
                n.as_x_string_node().map(|s| (s.opening_loc(), s.closing_loc()))
            }
            NodeKind::InterpolatedXStringNode => {
                n.as_interpolated_x_string_node().map(|s| (s.opening_loc(), s.closing_loc()))
            }
            _ => None,
        };
        if let Some((open, close)) = opening_closing {
            if ctx.text(open.span()).starts_with(b"<<") {
                out.push((open.span(), close.span()));
            }
        }
    };
    check(node);
    ruby_ast::each_descendant(node, &mut check);
}

/// Upstream's `heredoc.loc.heredoc_body.begin_pos`: the position right
/// after the opening tag's own physical line ends, since Prism's
/// `opening_loc` (unlike a dedicated `heredoc_body` location) only covers
/// the tag itself.
fn body_start(ctx: &Context<'_>, opening: Span) -> u32 {
    let source = ctx.source().bytes();
    let mut i = opening.end as usize;
    while i < source.len() && source[i] != b'\n' {
        i += 1;
    }
    u32::try_from((i + 1).min(source.len())).expect("offset exceeds u32")
}

/// Favor `File.(bin)write` convenience methods.
#[derive(Debug, Clone)]
pub struct FileWrite;

impl FileWrite {
    /// Reports `open_call`'s `filename`/`content` pair, replacing from
    /// `open_call`'s own `open` selector through `write_node_span`'s end
    /// with `method(filename, content)`, restoring any heredoc body that
    /// range would otherwise swallow.
    fn report(
        ctx: &mut Context<'_>,
        open_call: &CallNode<'_>,
        filename: &Node<'_>,
        content: &Node<'_>,
        mode: Mode,
        write_node_span: Span,
    ) {
        let method = mode.method();
        let message = format!("Use `File.{method}`.");
        let Some(selector_start) = open_call.message_loc().map(|l| l.span().start) else {
            return;
        };
        let range = Span::new(selector_start, write_node_span.end);

        let filename_src = String::from_utf8_lossy(ctx.text(filename.span())).into_owned();
        let content_src = String::from_utf8_lossy(ctx.text(content.span())).into_owned();
        let mut replacement = format!("{method}({filename_src}, {content_src})").into_bytes();

        let mut heredocs = Vec::new();
        collect_heredocs(ctx, filename, &mut heredocs);
        collect_heredocs(ctx, content, &mut heredocs);
        let removed: Vec<(Span, Span)> =
            heredocs.into_iter().filter(|(_, close)| close.end <= write_node_span.end).collect();
        if !removed.is_empty() {
            // Sorted by opening position (upstream's `heredoc_body.begin_pos`,
            // which orders identically): stacked heredocs sharing one
            // opening line all compute the same naive "after this line"
            // start, so each chunk's real start is clamped forward past the
            // previous heredoc's own close, matching how their bodies are
            // actually laid out back-to-back in the source.
            let mut sorted = removed;
            sorted.sort_by_key(|(open, _)| open.start);
            replacement.push(b'\n');
            let mut prev_close_end = 0u32;
            for (open, close) in sorted {
                let start = body_start(ctx, open).max(prev_close_end);
                replacement.extend_from_slice(ctx.text(Span::new(start, close.end)));
                prev_close_end = close.end;
            }
            // Prism's `closing_loc` includes the terminator's own trailing
            // newline (unlike whitequark's `heredoc_end`), which would
            // otherwise double up with the untouched source byte
            // immediately following `write_node_span` (upstream's
            // `write_node.source_range.end_pos` stops right before it too).
            if replacement.last() == Some(&b'\n') {
                replacement.pop();
            }
        }

        ctx.report_with_fix(
            &Self::META,
            write_node_span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, replacement)],
            },
        );
    }
}

impl Rule for FileWrite {
    const META: RuleMeta = RuleMeta {
        name: "Style/FileWrite",
        department: Department::Style,
        summary: "Favor `File.(bin)write` convenience methods.",
        explanation: "\
Favor `File.(bin)write` convenience methods.

NOTE: There are different method signatures between `File.write` (class method)
and `File#write` (instance method). The following case will be allowed because
static analysis does not know the contents of the splat argument:

```ruby
File.open(filename, 'w') do |f|
  f.write(*objects)
end
```

```ruby
# bad - text mode
File.open(filename, 'w').write(content)
File.open(filename, 'w') do |f|
  f.write(content)
end

# good
File.write(filename, content)

# bad - binary mode
File.open(filename, 'wb').write(content)
File.open(filename, 'wb') do |f|
  f.write(content)
end

# good
File.binwrite(filename, content)
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };

        if let Some((filename, mode)) = file_open_match(&call) {
            if let Some(block) = call.block().and_then(|b| b.as_block_node()) {
                if let Some(content) = block_write_content(&block) {
                    Self::report(ctx, &call, &filename, &content, mode, node.span());
                }
            }
            return;
        }

        if let Some(content) = send_write_content(&call) {
            if let Some(inner) =
                call.receiver().filter(|r| r.as_call_node().is_some_and(|c| c.block().is_none()))
            {
                let inner = inner.as_call_node().expect("checked Some");
                if let Some((filename, mode)) = file_open_match(&inner) {
                    Self::report(ctx, &inner, &filename, &content, mode, node.span());
                }
            }
        }
    }
}

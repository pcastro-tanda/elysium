//! `Style/FileRead`, ported from RuboCop's
//! `lib/rubocop/cop/style/file_read.rb`.
//!
//! Upstream matches `File.open(...)` with an attached block via
//! `node.parent` (whitequark wraps a block call as a separate `:block`
//! node, the inner `send`'s parent); Prism instead embeds a block directly
//! in the owning `CallNode`'s own `block()` field, so [`enter`] checks
//! `call.block()` directly for the block-pass (`&:read`) and literal-block
//! (`{ |f| f.read }`) shapes. The chained `.read` shape (no block at all)
//! has no such field to inspect from the `File.open` side, so it is instead
//! matched from the *outer* `.read` call's own perspective -- that call is
//! visited in its own right, and its `receiver()` gives direct access to
//! the inner `File.open(...)` call's fields without needing a parent
//! pointer.

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

/// Upstream's `READ_FILE_START_TO_FINISH_MODES`.
const READ_MODES: &[&[u8]] = &[b"r", b"rt", b"rb", b"r+", b"r+t", b"r+b"];

/// The read method `File.open(..., mode)` resolves to, computed eagerly
/// from the mode string so no borrow of it needs to escape
/// [`file_open_match`] (a `StringNode`'s `unescaped()` bytes do not outlive
/// the node value itself).
#[derive(Debug, Clone, Copy)]
enum Mode {
    /// No mode argument at all (upstream's implicit `'r'` default).
    Default,
    Read,
    Binread,
}

impl Mode {
    const fn method(self) -> &'static str {
        match self {
            Self::Binread => "binread",
            Self::Default | Self::Read => "read",
        }
    }
}

/// Upstream's `file_open?`: a plain (non-safe-navigation) `File.open(filename[,
/// mode])` call, `mode` being one of [`READ_MODES`] if given at all. Returns
/// the filename argument and the resolved read method.
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
    match args.as_slice() {
        [filename] => Some((*filename, Mode::Default)),
        [filename, mode] => {
            let string_node = mode.as_string_node()?;
            let bytes = string_node.unescaped();
            if !READ_MODES.contains(&bytes) {
                return None;
            }
            let mode = if bytes.ends_with(b"b") { Mode::Binread } else { Mode::Read };
            Some((*filename, mode))
        }
        _ => None,
    }
}

/// Whether `call.block()` is upstream's `&:read` block-pass.
fn is_read_block_pass(call: &CallNode<'_>) -> bool {
    call.block().and_then(|b| b.as_block_argument_node()).is_some_and(|ba| {
        ba.expression().and_then(|e| e.as_symbol_node()).is_some_and(|s| s.unescaped() == b"read")
    })
}

/// Upstream's `block_read?`: a block with exactly one declared parameter,
/// whose sole statement calls `.read` (no arguments) on that same
/// parameter.
fn is_block_read(block: &BlockNode<'_>) -> bool {
    let Some(params) = block.parameters().and_then(|p| p.as_block_parameters_node()) else {
        return false;
    };
    if !params.locals().is_empty() {
        return false;
    }
    let Some(params) = params.parameters() else { return false };
    if !params.optionals().is_empty()
        || params.rest().is_some()
        || !params.posts().is_empty()
        || !params.keywords().is_empty()
        || params.keyword_rest().is_some()
        || params.block().is_some()
    {
        return false;
    }
    let requireds: Vec<Node<'_>> = params.requireds().iter().collect();
    let [required] = requireds.as_slice() else { return false };
    let Some(param_name) = required.as_required_parameter_node().map(|p| p.name()) else {
        return false;
    };

    let Some(stmts) = block.body().and_then(|b| b.as_statements_node()) else { return false };
    let body: Vec<Node<'_>> = stmts.body().iter().collect();
    let [stmt] = body.as_slice() else { return false };
    let Some(read_call) = stmt.as_call_node() else { return false };
    if read_call.is_safe_navigation()
        || read_call.name().as_slice() != b"read"
        || read_call.arguments().is_some()
    {
        return false;
    }
    read_call
        .receiver()
        .and_then(|r| r.as_local_variable_read_node())
        .is_some_and(|lvar| lvar.name().as_slice() == param_name.as_slice())
}

/// Upstream's `send_read?`: a plain, argument-less `.read` call.
fn is_send_read(call: &CallNode<'_>) -> bool {
    !call.is_safe_navigation() && call.name().as_slice() == b"read" && call.arguments().is_none()
}

/// Favor `File.(bin)read` convenience methods.
#[derive(Debug, Clone)]
pub struct FileRead;

impl FileRead {
    /// Reports `open_call` being read in full via `read_node_span` (either
    /// `open_call`'s own span again, for an attached `&:read`/literal
    /// block, or the outer chained `.read` call's span).
    fn report(
        ctx: &mut Context<'_>,
        open_call: &CallNode<'_>,
        filename: &Node<'_>,
        mode: Mode,
        read_node_span: Span,
    ) {
        let method = mode.method();
        let message = format!("Use `File.{method}`.");
        let Some(selector_start) = open_call.message_loc().map(|l| l.span().start) else {
            return;
        };
        let range = Span::new(selector_start, read_node_span.end);
        let filename_src = String::from_utf8_lossy(ctx.text(filename.span())).into_owned();
        let replacement = format!("{method}({filename_src})");

        ctx.report_with_fix(
            &Self::META,
            read_node_span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, replacement.into_bytes())],
            },
        );
    }
}

impl Rule for FileRead {
    const META: RuleMeta = RuleMeta {
        name: "Style/FileRead",
        department: Department::Style,
        summary: "Favor `File.(bin)read` convenience methods.",
        explanation: "\
Favor `File.(bin)read` convenience methods.

```ruby
# bad - text mode
File.open(filename).read
File.open(filename, &:read)
File.open(filename) { |f| f.read }
File.open(filename) do |f|
  f.read
end
File.open(filename, 'r').read
File.open(filename, 'r', &:read)
File.open(filename, 'r') do |f|
  f.read
end

# good
File.read(filename)

# bad - binary mode
File.open(filename, 'rb').read
File.open(filename, 'rb', &:read)
File.open(filename, 'rb') do |f|
  f.read
end

# good
File.binread(filename)
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
            let triggers = is_read_block_pass(&call)
                || call.block().and_then(|b| b.as_block_node()).is_some_and(|b| is_block_read(&b));
            if triggers {
                Self::report(ctx, &call, &filename, mode, node.span());
                return;
            }
        }

        if is_send_read(&call) {
            if let Some(inner_call) =
                call.receiver().filter(|r| r.as_call_node().is_some_and(|c| c.block().is_none()))
            {
                let inner_call = inner_call.as_call_node().expect("checked Some");
                if let Some((filename, mode)) = file_open_match(&inner_call) {
                    Self::report(ctx, &inner_call, &filename, mode, node.span());
                }
            }
        }
    }
}

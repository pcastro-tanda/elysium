//! `Rails/FilePath`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/file_path.rb`.

use std::fmt::Write as _;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name, is_bare_or_toplevel_const};
use ruby_ast::node::CallNode;
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// `ConfigurableEnforcedStyle`'s `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Slashes,
    Arguments,
}

/// Identifies usages of file path joining process to use `Rails.root.join`
/// clause.
#[derive(Debug, Clone)]
pub struct FilePath {
    style: Style,
}

impl Rule for FilePath {
    const META: RuleMeta = RuleMeta {
        name: "Rails/FilePath",
        department: Department::Rails,
        summary: "Use `Rails.root.join` for file path joining.",
        explanation: "Identifies usages of file path joining process to use `Rails.root.join` \
                      clause. It is used to add uniformity when joining paths.\n\nNOTE: This cop \
                      ignores leading slashes in string literal arguments for `Rails.root.join` \
                      and multiple slashes in string literal arguments for `Rails.root.join` and \
                      `File.join`.\n\n```ruby\n# EnforcedStyle: slashes (default)\n# bad\n\
                      Rails.root.join('app', 'models', 'goober')\n\n# good\n\
                      Rails.root.join('app/models/goober')\n\n# bad\n\
                      File.join(Rails.root, 'app/models/goober')\n\
                      \"#{Rails.root}/app/models/goober\"\n\n# good\n\
                      Rails.root.join('app/models/goober').to_s\n```\n\n```ruby\n\
                      # EnforcedStyle: arguments\n# bad\nRails.root.join('app/models/goober')\n\n\
                      # good\nRails.root.join('app', 'models', 'goober')\n\n# bad\n\
                      File.join(Rails.root, 'app/models/goober')\n\
                      \"#{Rails.root}/app/models/goober\"\n\n# good\n\
                      Rails.root.join('app', 'models', 'goober').to_s\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode, NodeKind::InterpolatedStringNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("slashes"),
            allowed: &["slashes", "arguments"],
            doc: "Whether `Rails.root.join` takes one slash-separated path or one argument per \
                  path segment.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "arguments" => Style::Arguments,
            _ => Style::Slashes,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::InterpolatedStringNode => self.on_dstr(node, ctx),
            NodeKind::CallNode => self.on_send(node, ctx),
            _ => {}
        }
    }
}

impl FilePath {
    fn on_dstr(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(string) = node.as_interpolated_string_node() else { return };
        if !contains_rails_root(node) {
            return;
        }
        let parts: Vec<Node<'_>> = string.parts().iter().collect();
        // `dstr_separated_by_colon?`
        if parts
            .iter()
            .skip(1)
            .any(|part| part.as_string_node().is_some() && ctx.text(part.span()).starts_with(b":"))
        {
            return;
        }
        let Some(index) = parts.iter().position(contains_rails_root) else { return };

        self.check_slash_after_rails_root(node, &parts, index, ctx);
        self.check_extension_after_rails_root_join(node, &parts, index, ctx);
    }

    fn check_slash_after_rails_root(
        &self,
        node: &Node<'_>,
        parts: &[Node<'_>],
        index: usize,
        ctx: &mut Context<'_>,
    ) {
        let Some(slash_node) = parts.get(index + 1) else { return };
        if slash_node.as_string_node().is_none() || !ctx.text(slash_node.span()).starts_with(b"/") {
            return;
        }
        let Some(root_node) = interpolated_first_statement(&parts[index]) else { return };
        let Some(root_call) = root_node.as_call_node() else { return };
        if root_call.is_safe_navigation() {
            return;
        }

        let mut argument_source = Vec::new();
        for part in &parts[index + 1..] {
            argument_source.extend_from_slice(ctx.text(part.span()));
        }
        let argument_source = argument_source.strip_prefix(b"/").unwrap_or(&argument_source);

        let mut edits = Vec::new();
        if root_call.name().as_slice() == b"join" {
            let Some(last) = arguments_of(&root_call).pop() else {
                self.register(ctx, node.span(), false, None);
                return;
            };
            let mut text = b", \"".to_vec();
            text.extend_from_slice(argument_source);
            text.push(b'"');
            edits.push(Edit::insert(last.span().end, text));
        } else {
            let mut text = b"Rails.root.join(\"".to_vec();
            text.extend_from_slice(argument_source);
            text.extend_from_slice(b"\")");
            edits.push(Edit::replace(call_span_excluding_block(&root_call), text));
        }
        for part in &parts[index + 1..] {
            edits.push(Edit::delete(part.span()));
        }
        self.register(ctx, node.span(), false, Some(edits));
    }

    fn check_extension_after_rails_root_join(
        &self,
        node: &Node<'_>,
        parts: &[Node<'_>],
        index: usize,
        ctx: &mut Context<'_>,
    ) {
        let Some(extension_node) = parts.get(index + 1) else { return };
        if extension_node.as_string_node().is_none()
            || !is_extension(ctx.text(extension_node.span()))
        {
            return;
        }
        let edits = interpolated_first_statement(&parts[index]).and_then(|root_node| {
            let call = root_node.as_call_node()?;
            let last = arguments_of(&call).pop()?;
            let closing = last.as_string_node()?.closing_loc()?;
            Some(vec![
                Edit::insert(closing.span().start, ctx.text(extension_node.span()).to_vec()),
                Edit::delete(extension_node.span()),
            ])
        });
        self.register(ctx, node.span(), false, edits);
    }

    fn on_send(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"join" || call.is_safe_navigation() {
            return;
        }
        self.check_file_join_with_rails_root(&call, ctx);
        if call.receiver().is_none() {
            return;
        }
        if self.style == Style::Arguments {
            self.check_rails_root_join_with_slash_separated_path(&call, ctx);
        } else {
            self.check_rails_root_join_with_string_arguments(&call, ctx);
        }
    }

    fn check_file_join_with_rails_root(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        // `(send (const {nil? cbase} :File) :join ...)`
        let Some(receiver) = call.receiver() else { return };
        if !is_bare_or_toplevel_const(&receiver) || const_name(&receiver).as_deref() != Some("File")
        {
            return;
        }
        let arguments = arguments_of(call);
        if !arguments.iter().any(contains_rails_root)
            || arguments.iter().any(|argument| {
                is_variable(argument)
                    || is_const(argument)
                    || string_value(argument).is_some_and(|value| contains_multiple_slashes(&value))
            })
        {
            return;
        }

        let span = call_span_excluding_block(call);
        let edits = if arguments[0].as_array_node().is_some() {
            None
        } else {
            Some(file_join_edits(&receiver, &arguments, span, ctx))
        };
        self.register(ctx, span, true, edits);
    }

    fn check_rails_root_join_with_string_arguments(
        &self,
        call: &CallNode<'_>,
        ctx: &mut Context<'_>,
    ) {
        let Some(receiver) = call.receiver() else { return };
        if !contains_rails_root(&receiver) {
            return;
        }
        let arguments = arguments_of(call);
        if arguments.len() < 2 {
            return;
        }
        let mut values = Vec::new();
        for argument in &arguments {
            let Some(value) = string_value(argument) else { return };
            if value.starts_with(b"/") || contains_multiple_slashes(&value) {
                return;
            }
            values.push(value);
        }

        let mut joined = b"\"".to_vec();
        joined.extend_from_slice(&values.join(&b'/'));
        joined.push(b'"');
        let mut edits = vec![Edit::replace(arguments[0].span(), joined)];
        for argument in &arguments[1..] {
            let span = ctx.with_surrounding_space(argument.span(), Side::Left, true, false);
            edits.push(Edit::delete(comma_left(ctx.source().bytes(), span)));
        }
        self.register(ctx, call_span_excluding_block(call), false, Some(edits));
    }

    fn check_rails_root_join_with_slash_separated_path(
        &self,
        call: &CallNode<'_>,
        ctx: &mut Context<'_>,
    ) {
        let Some(receiver) = call.receiver() else { return };
        if !contains_rails_root(&receiver) {
            return;
        }
        let arguments = arguments_of(call);
        let slashed =
            |argument: &Node<'_>| string_value(argument).is_some_and(|value| value.contains(&b'/'));
        if !arguments.iter().any(slashed)
            || arguments.iter().any(|argument| {
                string_value(argument).is_some_and(|value| {
                    value.starts_with(b"/") || contains_multiple_slashes(&value)
                })
            })
        {
            return;
        }

        let mut edits = Vec::new();
        for argument in arguments.iter().filter(|argument| slashed(argument)) {
            let Some(string) = argument.as_string_node() else { return };
            let (Some(opening), Some(closing)) = (string.opening_loc(), string.closing_loc())
            else {
                self.register(ctx, call_span_excluding_block(call), false, None);
                return;
            };
            let source = ctx.text(argument.span());
            let Some(index) = source.iter().position(|&byte| byte == b'/') else {
                self.register(ctx, call_span_excluding_block(call), false, None);
                return;
            };
            // `inner_range_of(argument).adjust(begin_pos: index - 1)`
            let begin = (opening.span().end as usize + index).saturating_sub(1);
            let begin = u32::try_from(begin).expect("offset exceeds u32");
            let rest = Span::new(begin, closing.span().start);
            let mut text = b", \"".to_vec();
            let rest_source = ctx.text(rest);
            text.extend_from_slice(rest_source.strip_prefix(b"/").unwrap_or(rest_source));
            text.push(b'"');
            edits.push(Edit::delete(rest));
            edits.push(Edit::insert(argument.span().end, text));
        }
        self.register(ctx, call_span_excluding_block(call), false, Some(edits));
    }

    /// `register_offense`: the offense covers `node`'s first line from its
    /// column to its `last_column`.
    fn register(
        &self,
        ctx: &mut Context<'_>,
        node_span: Span,
        require_to_s: bool,
        edits: Option<Vec<Edit>>,
    ) {
        let span = first_line_range(ctx, node_span);
        let message = match (self.style, require_to_s) {
            (Style::Arguments, true) => "Prefer `Rails.root.join('path', 'to').to_s`.",
            (Style::Arguments, false) => "Prefer `Rails.root.join('path', 'to')`.",
            (Style::Slashes, true) => "Prefer `Rails.root.join('path/to').to_s`.",
            (Style::Slashes, false) => "Prefer `Rails.root.join('path/to')`.",
        };
        match edits {
            Some(edits) => ctx.report_with_fix(
                &Self::META,
                span,
                message,
                Fix { applicability: Applicability::Safe, edits },
            ),
            None => ctx.report(&Self::META, span, message),
        }
    }
}

/// `autocorrect_file_join`.
fn file_join_edits(
    receiver: &Node<'_>,
    arguments: &[Node<'_>],
    span: Span,
    ctx: &Context<'_>,
) -> Vec<Edit> {
    let bytes = ctx.source().bytes();
    let mut edits = vec![Edit::replace(receiver.span(), b"Rails.root".to_vec())];

    // `remove_first_argument_with_comma`
    let first = arguments[0].span();
    let with_comma = comma_right(bytes, first);
    edits.push(Edit::delete(ctx.with_surrounding_space(with_comma, Side::Right, true, false)));

    // `process_arguments`; the first argument is already swallowed by the
    // removal above.
    for argument in &arguments[1..] {
        if let Some(value) = string_value(argument) {
            let value = value.strip_prefix(b"/").unwrap_or(&value);
            let Ok(value) = std::str::from_utf8(value) else { continue };
            edits.push(Edit::replace(argument.span(), ruby_inspect(value).into_bytes()));
        } else if argument.as_array_node().is_some() {
            let mut text = b"*".to_vec();
            text.extend_from_slice(ctx.text(argument.span()));
            edits.push(Edit::replace(argument.span(), text));
        }
    }
    edits.push(Edit::insert(span.end, b".to_s".to_vec()));
    edits
}

/// Arguments of `call`, with a `&block` argument counted as one, as
/// whitequark's `send` does.
fn arguments_of<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut arguments: Vec<Node<'pr>> =
        call.arguments().map(|list| list.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block() {
        if block.as_block_argument_node().is_some() {
            arguments.push(block);
        }
    }
    arguments
}

/// `(send (const {nil? cbase} :Rails) :root)`.
fn is_rails_root(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.name().as_slice() != b"root" || call.is_safe_navigation() || call.arguments().is_some()
    {
        return false;
    }
    call.receiver().is_some_and(|receiver| {
        is_bare_or_toplevel_const(&receiver) && const_name(&receiver).as_deref() == Some("Rails")
    })
}

/// `rails_root_nodes?`: a node search, including `node` itself.
fn contains_rails_root(node: &Node<'_>) -> bool {
    let mut found = is_rails_root(node);
    if !found {
        each_descendant(node, &mut |child| found = found || is_rails_root(child));
    }
    found
}

/// `child.children.first` of an interpolation (`begin`) part.
fn interpolated_first_statement<'pr>(part: &Node<'pr>) -> Option<Node<'pr>> {
    part.as_embedded_statements_node()?.statements()?.body().iter().next()
}

/// `str_type?` plus `value`.
fn string_value(node: &Node<'_>) -> Option<Vec<u8>> {
    node.as_string_node().map(|string| string.unescaped().to_vec())
}

fn contains_multiple_slashes(value: &[u8]) -> bool {
    value.windows(2).any(|pair| pair == b"//")
}

/// `variable?`: `ivar`, `gvar`, `cvar` or `lvar`.
fn is_variable(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::LocalVariableReadNode
            | NodeKind::InstanceVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::GlobalVariableReadNode
            | NodeKind::ItLocalVariableReadNode
    )
}

fn is_const(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

/// `source.match?(/\A\.[A-Za-z]+/)`.
fn is_extension(source: &[u8]) -> bool {
    source.strip_prefix(b".").is_some_and(|rest| rest.first().is_some_and(u8::is_ascii_alphabetic))
}

/// `range_with_surrounding_comma(range, :right)`.
fn comma_right(bytes: &[u8], span: Span) -> Span {
    let mut end = span.end as usize;
    while bytes.get(end) == Some(&b',') {
        end += 1;
    }
    Span::new(span.start, u32::try_from(end).expect("offset exceeds u32"))
}

/// `range_with_surrounding_comma(range, :left)`.
fn comma_left(bytes: &[u8], span: Span) -> Span {
    let mut start = span.start as usize;
    while start > 0 && bytes[start - 1] == b',' {
        start -= 1;
    }
    Span::new(u32::try_from(start).expect("offset exceeds u32"), span.end)
}

/// `source_range(buffer, node.first_line, node.loc.column...node.loc.last_column)`.
fn first_line_range(ctx: &Context<'_>, span: Span) -> Span {
    let first = ctx.line_col(span.start);
    let last = ctx.line_col(span.end);
    let mut length = last.column.saturating_sub(first.column);
    let bytes = ctx.source().bytes();
    let mut end = span.start as usize;
    while length > 0 && end < bytes.len() {
        end += 1;
        while end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
            end += 1;
        }
        length -= 1;
    }
    Span::new(span.start, u32::try_from(end).expect("offset exceeds u32"))
}

/// `String#inspect` for a UTF-8 string.
fn ruby_inspect(value: &str) -> String {
    let mut out = String::from("\"");
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{c}' => out.push_str("\\f"),
            '\u{b}' => out.push_str("\\v"),
            '\u{8}' => out.push_str("\\b"),
            '\u{7}' => out.push_str("\\a"),
            '\u{1b}' => out.push_str("\\e"),
            '#' if matches!(chars.peek(), Some('{' | '$' | '@')) => out.push_str("\\#"),
            '\u{7f}' => out.push_str("\\x7F"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

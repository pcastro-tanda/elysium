//! `Rails/IndexBy`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/index_by.rb` and its `IndexMethod` mixin
//! (`lib/rubocop/cop/mixin/index_method.rb`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name, is_bare_or_toplevel_const};
use ruby_ast::node::{CallNode, ParametersNode};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const NEW_METHOD_NAME: &str = "index_by";

/// Looks for uses of `each_with_object({}) { ... }`, `map { ... }.to_h`, and
/// `Hash[map { ... }]` that transform an enumerable into a hash whose values
/// are the original elements.
#[derive(Debug, Clone)]
pub struct IndexBy {
    target_ruby_version: f32,
    /// `ignore_node` ranges of nodes already reported.
    ignored: Vec<Span>,
}

/// The offense description and how many characters of the whole node the
/// correction strips at each end (`Autocorrection#leading`/`#trailing`).
#[derive(Clone, Copy)]
struct Strip {
    description: &'static str,
    /// The offending node's span (a `send` excludes its own block).
    span: Span,
    leading: usize,
    trailing: usize,
}

/// What `IndexMethod` captures plus what the autocorrection needs.
struct Matched {
    /// `Captures#noop_transformation?`.
    noop: bool,
    /// Where the new method name goes: the selector (through the closing
    /// parenthesis when there is one) of the block's call.
    method_span: Span,
    /// The block parameters `|el|`, replaced by `|{arg_name}|` (regular
    /// blocks only).
    args: Option<(Span, String)>,
    /// The block body, replaced by the key expression.
    body_span: Span,
    /// The key expression, copied into the body's place.
    key_span: Span,
    /// `{ ... }` is added around a brace-less hash key.
    wrap_key: bool,
}

impl Rule for IndexBy {
    const META: RuleMeta = RuleMeta {
        name: "Rails/IndexBy",
        department: Department::Rails,
        summary: "Prefer `index_by` over `each_with_object`, `to_h`, or `map`.",
        explanation: "Looks for uses of `each_with_object({}) { ... }`, `map { ... }.to_h`, and \
                      `Hash[map { ... }]` that are transforming an enumerable into a hash where \
                      the values are the original elements. Rails provides the `index_by` \
                      method for this purpose.\n\n```ruby\n# bad\n[1, 2, 3].each_with_object({}) \
                      { |el, h| h[foo(el)] = el }\n[1, 2, 3].to_h { |el| [foo(el), el] }\n\
                      [1, 2, 3].map { |el| [foo(el), el] }.to_h\nHash[[1, 2, 3].collect { |el| \
                      [foo(el), el] }]\n\n# good\n[1, 2, 3].index_by { |el| foo(el) }\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version(), ignored: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let block = call.block().and_then(|b| b.as_block_node().map(|_| b));
        if block.is_some() {
            // `on_block` / `on_numblock` / `on_itblock`.
            if call.name().as_slice() == b"each_with_object" {
                if let Some(matched) = each_with_object(&call) {
                    self.handle(
                        &matched,
                        Strip {
                            description: "each_with_object",
                            span: node.span(),
                            leading: 0,
                            trailing: 0,
                        },
                        ctx,
                    );
                }
            }
            if self.target_ruby_version >= 2.6 && call.name().as_slice() == b"to_h" {
                if let Some(matched) = array_block(&call, &[b"to_h"]) {
                    self.handle(
                        &matched,
                        Strip {
                            description: "to_h { ... }",
                            span: node.span(),
                            leading: 0,
                            trailing: 0,
                        },
                        ctx,
                    );
                }
            }
        }
        // `on_send` / `on_csend`.
        let send_span = call_span_excluding_block(&call);
        match call.name().as_slice() {
            b"to_h" => {
                if let Some((matched, trailing)) = map_to_h(&call, node) {
                    self.handle(
                        &matched,
                        Strip {
                            description: "map { ... }.to_h",
                            span: send_span,
                            leading: 0,
                            trailing,
                        },
                        ctx,
                    );
                }
            }
            b"[]" if !call.is_safe_navigation() => {
                if let Some((matched, leading)) = hash_brackets_map(&call, ctx) {
                    self.handle(
                        &matched,
                        Strip {
                            description: "Hash[map { ... }]",
                            span: send_span,
                            leading,
                            trailing: 1,
                        },
                        ctx,
                    );
                }
            }
            _ => {}
        }
    }
}

impl IndexBy {
    fn handle(&mut self, matched: &Matched, strip: Strip, ctx: &mut Context<'_>) {
        let Strip { description, span, leading, trailing } = strip;
        if matched.noop {
            return;
        }
        let message = format!("Prefer `{NEW_METHOD_NAME}` over `{description}`.");
        let part_of_ignored = self
            .ignored
            .iter()
            .any(|ignored| ignored.start <= span.start && ignored.end >= span.end);
        if part_of_ignored {
            ctx.report(&Self::META, span, message);
        } else {
            let mut edits = Vec::new();
            if leading > 0 {
                edits.push(Edit::delete(Span::new(span.start, span.start + to_u32(leading))));
            }
            if trailing > 0 {
                edits.push(Edit::delete(Span::new(span.end - to_u32(trailing), span.end)));
            }
            edits.push(Edit::replace(matched.method_span, NEW_METHOD_NAME.as_bytes().to_vec()));
            if let Some((args_span, name)) = &matched.args {
                edits.push(Edit::replace(*args_span, format!("|{name}|").into_bytes()));
            }
            let mut key = ctx.text(matched.key_span).to_vec();
            if matched.wrap_key {
                key = [b"{ ".as_slice(), &key, b" }"].concat();
            }
            edits.push(Edit::replace(matched.body_span, key));
            ctx.report_with_fix(
                &Self::META,
                span,
                message,
                Fix { applicability: Applicability::Safe, edits },
            );
        }
        self.ignored.push(span);
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).expect("offset exceeds u32")
}

/// The single statement of a block body.
fn single_body<'pr>(block: &Node<'pr>) -> Option<Node<'pr>> {
    let body = block.as_block_node()?.body()?;
    let statements = body.as_statements_node()?;
    let mut iter = statements.body().iter();
    let (Some(first), None) = (iter.next(), iter.next()) else { return None };
    Some(first)
}

fn method_span(call: &CallNode<'_>) -> Option<Span> {
    let selector = call.message_loc()?.span();
    Some(Span::new(selector.start, call.closing_loc().map_or(selector.end, |c| c.span().end)))
}

fn is_read_of(node: &Node<'_>, name: &[u8]) -> bool {
    node.as_local_variable_read_node().is_some_and(|read| read.name().as_slice() == name)
}

/// The `each_with_object({}) { |el, memo| memo[key] = el }` matcher, where the
/// key must not mention `memo`.
fn each_with_object(call: &CallNode<'_>) -> Option<Matched> {
    let arguments = call.arguments()?;
    let mut iter = arguments.arguments().iter();
    let (Some(arg), None) = (iter.next(), iter.next()) else { return None };
    let hash = arg.as_hash_node()?;
    if hash.elements().iter().next().is_some() {
        return None;
    }
    let block = call.block()?;
    let block_node = block.as_block_node()?;
    let parameters = block_node.parameters()?;
    let parameters = parameters.as_block_parameters_node()?;
    if parameters.locals().iter().next().is_some() {
        return None;
    }
    let params = parameters.parameters()?;
    let names = only_required(&params, 2)?;
    let (el, memo) = (&names[0], &names[1]);
    let body = single_body(&block)?;
    let assign = body.as_call_node()?;
    if assign.name().as_slice() != b"[]=" || assign.block().is_some() {
        return None;
    }
    if !assign.receiver().is_some_and(|r| is_read_of(&r, memo)) {
        return None;
    }
    let assign_args = assign.arguments()?;
    let mut iter = assign_args.arguments().iter();
    let (Some(key), Some(value), None) = (iter.next(), iter.next(), iter.next()) else {
        return None;
    };
    if !is_read_of(&value, el) {
        return None;
    }
    let mut mentions_memo = is_read_of(&key, memo);
    each_descendant(&key, &mut |n| mentions_memo |= is_read_of(n, memo));
    if mentions_memo {
        return None;
    }
    Some(Matched {
        noop: is_read_of(&key, el),
        method_span: method_span(call)?,
        args: Some((parameters.as_node().span(), String::from_utf8_lossy(el).into_owned())),
        body_span: body.span(),
        key_span: key.span(),
        wrap_key: false,
    })
}

/// The names of exactly `count` required parameters and nothing else.
fn only_required(params: &ParametersNode<'_>, count: usize) -> Option<Vec<Vec<u8>>> {
    if params.optionals().iter().next().is_some()
        || params.rest().is_some()
        || params.posts().iter().next().is_some()
        || params.keywords().iter().next().is_some()
        || params.keyword_rest().is_some()
        || params.block().is_some()
    {
        return None;
    }
    let names: Vec<Vec<u8>> = params
        .requireds()
        .iter()
        .map(|p| p.as_required_parameter_node().map(|p| p.name().as_slice().to_vec()))
        .collect::<Option<_>>()?;
    (names.len() == count).then_some(names)
}

/// `(array $_ (lvar _el))` blocks over `names` (`map`/`collect`/`to_h`):
/// regular, numbered-parameter (`_1`) and `it` blocks.
fn array_block(call: &CallNode<'_>, names: &[&[u8]]) -> Option<Matched> {
    if !names.contains(&call.name().as_slice()) || call.arguments().is_some() {
        return None;
    }
    let block = call.block()?;
    let block_node = block.as_block_node()?;
    let body = single_body(&block)?;
    let array = body.as_array_node()?;
    let mut elements = array.elements().iter();
    let (Some(key), Some(value), None) = (elements.next(), elements.next(), elements.next()) else {
        return None;
    };
    let parameters = block_node.parameters()?;
    let (noop, args) = if let Some(parameters) = parameters.as_block_parameters_node() {
        if parameters.locals().iter().next().is_some() {
            return None;
        }
        let params = parameters.parameters()?;
        let el = only_required(&params, 1)?.remove(0);
        if !is_read_of(&value, &el) {
            return None;
        }
        (
            is_read_of(&key, &el),
            Some((parameters.as_node().span(), String::from_utf8_lossy(&el).into_owned())),
        )
    } else if parameters.as_numbered_parameters_node().is_some_and(|p| p.maximum() == 1) {
        if !is_read_of(&value, b"_1") {
            return None;
        }
        (false, None)
    } else if parameters.as_it_parameters_node().is_some() {
        value.as_it_local_variable_read_node()?;
        (key.as_it_local_variable_read_node().is_some(), None)
    } else {
        return None;
    };
    Some(Matched {
        noop,
        method_span: method_span(call)?,
        args,
        body_span: body.span(),
        key_span: key.span(),
        wrap_key: key.as_keyword_hash_node().is_some(),
    })
}

/// `(call (block (call _ {:map :collect}) ...) :to_h)`; also returns how many
/// trailing characters (`.to_h`) the correction strips.
fn map_to_h(call: &CallNode<'_>, node: &Node<'_>) -> Option<(Matched, usize)> {
    if call.arguments().is_some()
        || call.block().is_some_and(|b| b.as_block_argument_node().is_some())
    {
        return None;
    }
    let receiver = call.receiver()?;
    let map = receiver.as_call_node()?;
    let matched = array_block(&map, &[b"map", b"collect"])?;
    // `node.block_literal?` keeps the `.to_h` when it has a block of its own.
    let trailing =
        if call.block().is_some() { 0 } else { (node.span().end - receiver.span().end) as usize };
    Some((matched, trailing))
}

/// `(send (const {nil? cbase} :Hash) :[] (block (call _ {:map :collect}) ...))`;
/// also returns the length of the `Hash[` prefix the correction strips.
fn hash_brackets_map(call: &CallNode<'_>, ctx: &Context<'_>) -> Option<(Matched, usize)> {
    if call.block().is_some() {
        return None;
    }
    let receiver = call.receiver()?;
    if !is_bare_or_toplevel_const(&receiver) || const_name(&receiver).as_deref() != Some("Hash") {
        return None;
    }
    let arguments = call.arguments()?;
    let mut iter = arguments.arguments().iter();
    let (Some(argument), None) = (iter.next(), iter.next()) else { return None };
    let map = argument.as_call_node()?;
    let matched = array_block(&map, &[b"map", b"collect"])?;
    let leading = ctx.text(receiver.span()).len() + "[".len();
    Some((matched, leading))
}

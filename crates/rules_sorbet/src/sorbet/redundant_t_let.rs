//! `Sorbet/RedundantTLet`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/redundant_t_let.rb` (with its `SignatureHelp`,
//! `ConstantScope`, `TargetSorbetVersion` and `TLetCorrection` mixins).

use std::sync::LazyLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, GemVersion, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::ext::is_heredoc;
use ruby_ast::node::CallNode;
use ruby_ast::{each_descendant, for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Unnecessary T.let. The instance variable type is inferred from the signature.";
const MSG_CONSTRUCTOR: &str =
    "Unnecessary T.let. The constant type is inferred from the constructor.";

/// Classes whose constructor calls Sorbet infers as applied generic types
/// (e.g. `T::Set[T.untyped]`) rather than the bare class, so constants
/// assigned them still need an explicit annotation.
const GENERIC_CLASSES: [&str; 7] =
    ["Array", "Class", "Enumerator", "Hash", "Module", "Range", "Set"];

/// `minimum_target_sorbet_static_version`.
const MINIMUM_SORBET_STATIC_VERSION: &str = "0.6.13304";

/// Prevents unnecessary `T.let` where Sorbet infers the type automatically.
#[derive(Debug, Clone)]
pub struct RedundantTLet {
    /// `enabled_for_sorbet_static_version?`: `sorbet-static` is locked and at
    /// least the minimum version.
    enabled_for_sorbet_static_version: bool,
}

impl Rule for RedundantTLet {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/RedundantTLet",
        department: Department::Sorbet,
        summary: "Prevents redundant use of `T.let` where Sorbet infers types automatically: instance variables assigned from signature parameters in `initialize`, and constants assigned constructor calls.",
        explanation: "Prevents unnecessary `T.let` where Sorbet infers the type automatically.\n\n\
                      When a signature parameter is assigned to an instance variable in \
                      `initialize`, the type is inferred from the signature.\n\n\
                      When a constant is assigned a constructor call (`.new`), optionally \
                      followed by `.freeze` (Sorbet 0.6.13304+), the type is inferred from the \
                      class being instantiated. Generic classes (e.g. `Set`) are excluded: \
                      Sorbet infers their constructor calls as applied types like \
                      `T::Set[T.untyped]`, so an annotation is still required.\n\n```ruby\n\
                      # bad\nsig { params(a: Integer) }\ndef initialize(a)\n  @a = T.let(a, \
                      Integer)\nend\n\n# good\nsig { params(a: Integer) }\ndef initialize(a)\n  \
                      @a = a\nend\n\n# good\nsig { params(a: Integer) }\ndef initialize(a)\n  \
                      @a = T.let(a, T.any(Integer, String))\nend\n\n# bad\nDEFAULT_PATH = \
                      T.let(Pathname.new(\"/usr/local\").freeze, Pathname)\n\n# good\n\
                      DEFAULT_PATH = Pathname.new(\"/usr/local\").freeze\n\n# good — generic \
                      classes are not inferred, so T.let is required\nLICENSES = \
                      T.let(Set.new([\"mit\"]).freeze, T::Set[String])\n\n# good — instance \
                      variables are only inferred from signature parameters\n@path = \
                      T.let(Pathname.new(\"/usr/local\"), Pathname)\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::StatementsNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let minimum = GemVersion::parse(MINIMUM_SORBET_STATIC_VERSION);
        let enabled = match (options.gem_version("sorbet-static"), minimum) {
            (Some(version), Some(minimum)) => minimum <= version,
            _ => false,
        };
        Ok(Self { enabled_for_sorbet_static_version: enabled })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::StatementsNode => Self::on_statements(node, ctx),
            NodeKind::ConstantWriteNode => {
                if let Some(write) = node.as_constant_write_node() {
                    self.on_casgn(node, &write.value(), ctx);
                }
            }
            NodeKind::ConstantPathWriteNode => {
                if let Some(write) = node.as_constant_path_write_node() {
                    self.on_casgn(node, &write.value(), ctx);
                }
            }
            _ => {}
        }
    }
}

impl RedundantTLet {
    /// `on_def`, reached through the `StatementsNode` holding the `def` so its
    /// `left_sibling` is at hand. A `def` that is not a statement (e.g. the
    /// argument of `private def initialize`) has a `send` parent, which
    /// `find_sig_node` rejects.
    fn on_statements(statements: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(statements) = statements.as_statements_node() else { return };
        let body: Vec<Node<'_>> = statements.body().iter().collect();
        for (index, statement) in body.iter().enumerate() {
            let Some(def) = statement.as_def_node() else { continue };
            if def.name().as_slice() != b"initialize" {
                continue;
            }

            let method_args = named_argument_types(&def);
            if method_args.is_empty() {
                continue;
            }

            // `find_sig_node`: `method_node.left_sibling` when it is a signature.
            let Some(sig_node) = index.checked_sub(1).map(|previous| body[previous]) else {
                continue;
            };
            if !is_signature(&sig_node) {
                continue;
            }

            let Some(sig_params) = sig_params(&sig_node) else { continue };
            if sig_params.is_empty() {
                continue;
            }

            for ivasgn in ivar_assignments(&def) {
                let Some(write) = ivasgn.as_instance_variable_write_node() else { continue };
                // `(ivasgn _ $(send (const {nil? cbase} :T) :let (lvar $_) $_))`
                let value = write.value();
                let Some(call) = value.as_call_node() else { continue };
                let Some((first, second)) = t_let_arguments(&call) else { continue };
                let Some(lvar) = first.as_local_variable_read_node() else { continue };
                find_redundant_t_let(
                    ctx,
                    &value,
                    lvar.name().as_slice(),
                    &second,
                    &sig_params,
                    &method_args,
                );
            }
        }
    }

    fn on_casgn(&self, node: &Node<'_>, value: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled_for_sorbet_static_version {
            return;
        }
        // `(casgn _ _ $(send (const {nil? cbase} :T) :let $_ $_))`
        let Some(call) = value.as_call_node() else { return };
        let Some((value_node, type_node)) = t_let_arguments(&call) else { return };

        // `next unless type_node.const_type?`
        if type_node.as_constant_read_node().is_none()
            && type_node.as_constant_path_node().is_none()
        {
            return;
        }
        let root = ctx.parsed().root();
        let mut path = Vec::new();
        if !find_path(&root, node, &mut path) || !statically_scoped(&path) {
            return;
        }
        let Some(constructor) = constructor_call(&value_node) else { return };
        let Some(receiver) = constructor.receiver() else { return };

        let class_path = delete_prefix_cbase(ctx.text(receiver.span()));
        if GENERIC_CLASSES.iter().any(|generic| generic.as_bytes() == class_path) {
            return;
        }
        if class_path != delete_prefix_cbase(ctx.text(type_node.span())) {
            return;
        }

        let fix = replace_t_let(ctx, value, &value_node);
        ctx.report_with_fix(
            &Self::META,
            value.span(),
            MSG_CONSTRUCTOR,
            Fix { applicability: Applicability::Unsafe, edits: vec![fix] },
        );
    }
}

fn delete_prefix_cbase(text: &[u8]) -> &[u8] {
    text.strip_prefix(b"::").unwrap_or(text)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ArgKind {
    Arg,
    RestArg,
    KwRestArg,
}

/// Maps named parameters to their kinds, omitting destructured parameters
/// (and `...`, `**nil`) which respond to no `name`. Anonymous `*`/`**`/`&`
/// have a `nil` name but still count.
fn named_argument_types(def: &ruby_ast::node::DefNode<'_>) -> Vec<(Option<Vec<u8>>, ArgKind)> {
    let mut args = Vec::new();
    let Some(parameters) = def.parameters() else { return args };
    let named =
        |name: Option<ruby_ast::node::ConstantId<'_>>| name.map(|name| name.as_slice().to_vec());
    let plain = |node: &Node<'_>| -> Option<Vec<u8>> {
        if let Some(arg) = node.as_required_parameter_node() {
            Some(arg.name().as_slice().to_vec())
        } else {
            node.as_optional_parameter_node().map(|arg| arg.name().as_slice().to_vec())
        }
    };
    for node in &parameters.requireds() {
        if let Some(name) = plain(&node) {
            args.push((Some(name), ArgKind::Arg));
        }
    }
    for node in &parameters.optionals() {
        if let Some(name) = plain(&node) {
            args.push((Some(name), ArgKind::Arg));
        }
    }
    if let Some(rest) = parameters.rest().and_then(|rest| rest.as_rest_parameter_node()) {
        args.push((named(rest.name()), ArgKind::RestArg));
    }
    for node in &parameters.posts() {
        if let Some(name) = plain(&node) {
            args.push((Some(name), ArgKind::Arg));
        }
    }
    for node in &parameters.keywords() {
        if let Some(keyword) = node.as_required_keyword_parameter_node() {
            args.push((Some(keyword.name().as_slice().to_vec()), ArgKind::Arg));
        } else if let Some(keyword) = node.as_optional_keyword_parameter_node() {
            args.push((Some(keyword.name().as_slice().to_vec()), ArgKind::Arg));
        }
    }
    if let Some(rest) =
        parameters.keyword_rest().and_then(|rest| rest.as_keyword_rest_parameter_node())
    {
        args.push((named(rest.name()), ArgKind::KwRestArg));
    }
    if let Some(block) = parameters.block() {
        args.push((named(block.name()), ArgKind::Arg));
    }
    args
}

/// `ivar_assignments`: the `ivasgn` statements of the body (a `begin`'s
/// children, or the lone statement); nothing for a rescue/ensure body.
fn ivar_assignments<'pr>(def: &ruby_ast::node::DefNode<'pr>) -> Vec<Node<'pr>> {
    let Some(body) = def.body() else { return Vec::new() };
    let Some(statements) = body.as_statements_node() else { return Vec::new() };
    statements
        .body()
        .iter()
        .filter(|statement| statement.as_instance_variable_write_node().is_some())
        .collect()
}

fn find_redundant_t_let(
    ctx: &mut Context<'_>,
    tlet_node: &Node<'_>,
    tlet_key: &[u8],
    tlet_value: &Node<'_>,
    sig_params: &[(Option<Vec<u8>>, Span)],
    method_args: &[(Option<Vec<u8>>, ArgKind)],
) {
    let key = Some(tlet_key);
    let Some(sig_type) = sig_params.iter().rev().find(|(name, _)| name.as_deref() == key) else {
        return;
    };
    let Some((_, kind)) = method_args.iter().rev().find(|(name, _)| name.as_deref() == key) else {
        return;
    };

    let sig_source = normalize_whitespace(&String::from_utf8_lossy(ctx.text(sig_type.1)));
    let arg_type = match kind {
        ArgKind::RestArg => format!("T::Array[{sig_source}]"),
        ArgKind::KwRestArg => format!("T::Hash[Symbol, {sig_source}]"),
        ArgKind::Arg => sig_source,
    };
    let value_source = normalize_whitespace(&String::from_utf8_lossy(ctx.text(tlet_value.span())));
    if value_source != arg_type {
        return;
    }

    let span = tlet_node.span();
    ctx.report_with_fix(
        &RedundantTLet::META,
        span,
        MSG,
        Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(span, tlet_key.to_vec())],
        },
    );
}

macro_rules! regex {
    ($pattern:expr) => {{
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new($pattern).expect("valid regex"));
        &*RE
    }};
}

fn normalize_whitespace(source: &str) -> String {
    // `\s` in Ruby is `[ \t\r\n\f\v]`.
    let collapsed = regex!(r"[ \t\r\n\x0c\x0b]+").replace_all(source, " ");
    let no_trailing_commas = regex!(r",[ \t\r\n\x0c\x0b]*([)\]}])").replace_all(&collapsed, "$1");
    let comma_spacing = regex!(r",[ \t\r\n\x0c\x0b]*").replace_all(&no_trailing_commas, ", ");
    let after_paren = regex!(r"\([ \t\r\n\x0c\x0b]*").replace_all(&comma_spacing, "(");
    let before_paren = regex!(r"[ \t\r\n\x0c\x0b]*\)").replace_all(&after_paren, ")");
    let after_bracket = regex!(r"\[[ \t\r\n\x0c\x0b]*").replace_all(&before_paren, "[");
    let before_bracket = regex!(r"[ \t\r\n\x0c\x0b]*\]").replace_all(&after_bracket, "]");
    before_bracket.trim_matches([' ', '\t', '\r', '\n', '\x0c', '\x0b', '\0']).to_owned()
}

fn is_const_t(node: &Node<'_>) -> bool {
    // `(const {nil? cbase} :T)`
    if let Some(constant) = node.as_constant_read_node() {
        return constant.name().as_slice() == b"T";
    }
    node.as_constant_path_node().is_some_and(|path| {
        path.parent().is_none() && path.name().is_some_and(|name| name.as_slice() == b"T")
    })
}

/// `send` arguments as whitequark sees them: a `&blk` is a `block_pass` child.
fn argument_list<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut args: Vec<Node<'pr>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
        args.push(block);
    }
    args
}

/// A whitequark `send`: neither `csend` nor a call that carries a literal
/// block (which becomes a `block` node).
fn is_send(call: &CallNode<'_>) -> bool {
    !call.is_safe_navigation() && call.block().is_none_or(|b| b.as_block_node().is_none())
}

/// `(send (const {nil? cbase} :T) :let $_ $_)`.
fn t_let_arguments<'pr>(call: &CallNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    if call.name().as_slice() != b"let" || !is_send(call) || !is_const_t(&call.receiver()?) {
        return None;
    }
    match argument_list(call).as_slice() {
        [first, second] => Some((*first, *second)),
        _ => None,
    }
}

/// `constructor_call`: the `.new` call of a constructor, looking through an
/// optional block and a trailing `.freeze`.
fn constructor_call<'pr>(value: &Node<'pr>) -> Option<CallNode<'pr>> {
    let mut node = Some(*value);
    if let Some(call) = value.as_call_node() {
        if is_send(&call) && call.name().as_slice() == b"freeze" {
            node = call.receiver();
        }
    }
    // `unwrap_block`: a block's call is the call itself in Prism.
    let call = node?.as_call_node()?;
    if call.is_safe_navigation() || call.name().as_slice() != b"new" {
        return None;
    }
    let receiver = call.receiver()?;
    if receiver.as_constant_read_node().is_none() && receiver.as_constant_path_node().is_none() {
        return None;
    }
    Some(call)
}

/// `signature?`: `bare_sig?`, `sig_with_runtime?` or `sig_without_runtime?`.
fn is_signature(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.name().as_slice() != b"sig" || call.is_safe_navigation() {
        return false;
    }
    // `(block (send ... :sig (sym :final)?) (args) ...)`
    let Some(block) = call.block().and_then(|block| block.as_block_node()) else { return false };
    let empty_args = match block.parameters() {
        None => true,
        Some(parameters) => parameters.as_block_parameters_node().is_some_and(|parameters| {
            parameters.parameters().is_none() && parameters.locals().iter().next().is_none()
        }),
    };
    if !empty_args {
        return false;
    }
    let arguments: Vec<Node<'_>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    let final_arg = match arguments.as_slice() {
        [] => true,
        [one] => one.as_symbol_node().is_some_and(|symbol| symbol.unescaped() == b"final"),
        _ => false,
    };
    if !final_arg {
        return false;
    }
    match call.receiver() {
        None => true,
        Some(receiver) => is_t_sig(&receiver, 0) || is_t_sig(&receiver, 1),
    }
}

/// `(const (const {nil? cbase} :T) :Sig)` (`extra == 0`) or the same followed
/// by `:WithoutRuntime` (`extra == 1`).
fn is_t_sig(node: &Node<'_>, extra: u8) -> bool {
    let Some(path) = node.as_constant_path_node() else { return false };
    let Some(parent) = path.parent() else { return false };
    let Some(name) = path.name() else { return false };
    if extra == 1 {
        return name.as_slice() == b"WithoutRuntime" && is_t_sig(&parent, 0);
    }
    name.as_slice() == b"Sig" && is_const_t(&parent)
}

/// `` `(send nil? :params (hash $...)) ``: the first matching descendant's
/// pairs, as `(symbol name, value span)`.
fn sig_params(sig: &Node<'_>) -> Option<Vec<(Option<Vec<u8>>, Span)>> {
    let mut found: Option<Vec<(Option<Vec<u8>>, Span)>> = None;
    let mut visit = |candidate: &Node<'_>| {
        if found.is_some() {
            return;
        }
        let Some(call) = candidate.as_call_node() else { return };
        if call.name().as_slice() != b"params"
            || call.receiver().is_some()
            || call.is_safe_navigation()
        {
            return;
        }
        let arguments = argument_list(&call);
        let [hash] = arguments.as_slice() else { return };
        let elements: Vec<Node<'_>> = if let Some(hash) = hash.as_keyword_hash_node() {
            hash.elements().iter().collect()
        } else if let Some(hash) = hash.as_hash_node() {
            hash.elements().iter().collect()
        } else {
            return;
        };
        let mut pairs = Vec::new();
        for element in &elements {
            // A `**splat` has no `key`; upstream would raise.
            let Some(pair) = element.as_assoc_node() else { return };
            let key = pair.key().as_symbol_node().map(|key| key.unescaped().to_vec());
            pairs.push((key, pair.value().span()));
        }
        found = Some(pairs);
    };
    // The `block` node: its `send`, then the body.
    visit(sig);
    each_descendant(sig, &mut visit);
    found
}

/// The Prism ancestors (outermost first) of `target`.
fn find_path<'pr>(node: &Node<'pr>, target: &Node<'_>, path: &mut Vec<Node<'pr>>) -> bool {
    if node.span() == target.span() && node.kind() == target.kind() {
        return true;
    }
    let (span, wanted) = (node.span(), target.span());
    if span.start > wanted.start || span.end < wanted.end {
        return false;
    }
    path.push(*node);
    let mut found = false;
    for_each_child(node, |child| {
        if !found && find_path(child, target, path) {
            found = true;
        }
    });
    if !found {
        path.pop();
    }
    found
}

/// `ConstantScope#statically_scoped?`, over whitequark's ancestors: a
/// `StatementsNode` is a `begin` only with several statements and no
/// `ParenthesesNode` around it (the parentheses are the `begin` then).
fn statically_scoped(path: &[Node<'_>]) -> bool {
    #[derive(Clone, Copy, PartialEq)]
    enum Ancestor {
        Begin,
        Scope,
        Other,
    }
    let mut chain = Vec::with_capacity(2);
    for index in (0..path.len()).rev() {
        if chain.len() == 2 {
            break;
        }
        let ancestor = &path[index];
        match ancestor.kind() {
            NodeKind::ArgumentsNode | NodeKind::ProgramNode => {}
            NodeKind::StatementsNode => {
                let in_parentheses = index
                    .checked_sub(1)
                    .is_some_and(|outer| path[outer].kind() == NodeKind::ParenthesesNode);
                let several = ancestor
                    .as_statements_node()
                    .is_some_and(|statements| statements.body().iter().count() >= 2);
                if several && !in_parentheses {
                    chain.push(Ancestor::Begin);
                }
            }
            NodeKind::ParenthesesNode => chain.push(Ancestor::Begin),
            NodeKind::ClassNode | NodeKind::ModuleNode | NodeKind::SingletonClassNode => {
                chain.push(Ancestor::Scope);
            }
            _ => chain.push(Ancestor::Other),
        }
    }
    let mut ancestors = chain.into_iter();
    let mut ancestor = ancestors.next();
    if ancestor == Some(Ancestor::Begin) {
        ancestor = ancestors.next();
    }
    ancestor.is_none() || ancestor == Some(Ancestor::Scope)
}

/// The terminator of a heredoc (`loc.heredoc_end`), without its newline.
fn heredoc_end(ctx: &Context<'_>, node: &Node<'_>) -> Option<Span> {
    let closing = match node.kind() {
        NodeKind::StringNode => node.as_string_node()?.closing_loc(),
        NodeKind::InterpolatedStringNode => node.as_interpolated_string_node()?.closing_loc(),
        NodeKind::XStringNode => node.as_x_string_node().map(|n| n.closing_loc()),
        NodeKind::InterpolatedXStringNode => {
            node.as_interpolated_x_string_node().map(|n| n.closing_loc())
        }
        _ => None,
    }?;
    let mut span = closing.span();
    let text = ctx.text(span);
    let trimmed = text.strip_suffix(b"\n").unwrap_or(text);
    let trimmed = trimmed.strip_suffix(b"\r").unwrap_or(trimmed);
    span.end = span.start + u32::try_from(trimmed.len()).expect("offset exceeds u32");
    Some(span)
}

/// `TLetCorrection#replace_t_let`: replaces `T.let(value, Type)` with `value`,
/// keeping the bodies of heredocs that open in the value but end after it.
fn replace_t_let(ctx: &Context<'_>, t_let_node: &Node<'_>, value_node: &Node<'_>) -> Edit {
    let t_let_span = t_let_node.span();
    let value_span = value_node.span();

    // `tail_heredocs`
    let mut heredocs: Vec<(Span, Span)> = Vec::new();
    let mut collect = |node: &Node<'_>| {
        if is_heredoc(node) {
            if let Some(end) = heredoc_end(ctx, node) {
                if end.end > value_span.end {
                    heredocs.push((node.span(), end));
                }
            }
        }
    };
    collect(value_node);
    each_descendant(value_node, &mut collect);

    if heredocs.is_empty() {
        return Edit::replace(t_let_span, ctx.text(value_span).to_vec());
    }

    let source = ctx.source().bytes();
    // `range_including_heredocs`
    let end = heredocs.iter().map(|(_, end)| end.end).fold(t_let_span.end, u32::max);

    // `source_including_heredocs`
    heredocs.sort_by_key(|(opening, _)| opening.start);
    let mut text = ctx.text(value_span).to_vec();
    text.extend_from_slice(&marker_line_comment(source, value_span.end as usize));

    // Bodies follow the line holding the markers, one after another.
    let mut cursor = value_span.end as usize;
    let mut body_start = source[cursor..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(source.len(), |offset| cursor + offset + 1);
    for (_, terminator) in &heredocs {
        text.push(b'\n');
        text.extend_from_slice(&source[body_start..terminator.end as usize]);
        cursor = terminator.end as usize;
        body_start = source[cursor..]
            .iter()
            .position(|&byte| byte == b'\n')
            .map_or(source.len(), |offset| cursor + offset + 1);
    }
    Edit::replace(Span::new(t_let_span.start, end), text)
}

/// The comment trailing the heredoc marker, with its leading space.
fn marker_line_comment(source: &[u8], value_end: usize) -> Vec<u8> {
    let line_end = source[value_end..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(source.len(), |offset| value_end + offset);
    let rest = &source[value_end..line_end];
    match rest.iter().position(|&byte| byte == b'#') {
        Some(index) => {
            let mut comment = vec![b' '];
            comment.extend_from_slice(&rest[index..]);
            comment
        }
        None => Vec::new(),
    }
}

//! `Lint/ToEnumArguments`, ported from RuboCop's
//! `lib/rubocop/cop/lint/to_enum_arguments.rb`.
//!
//! Upstream reads the enclosing method's parameter list flattened as a
//! single `node.arguments` array (whitequark `arg`/`optarg`/`restarg`/
//! `kwarg`/`kwoptarg`/`kwrestarg`/`forward_arg`), found via
//! `node.each_ancestor(:any_def).first`. [`Context::ancestors`] only
//! exposes `NodeInfo` (kind/span, no typed field access -- see
//! `suppressed_exception.rs`'s module doc for the same constraint), and
//! [`Rule`] itself requires `'static` state, so a borrowed [`DefNode`]
//! can't be cached across `enter` calls either. This port instead
//! subscribes to [`NodeKind::DefNode`] directly and pushes/pops an owned,
//! pre-extracted [`DefFrame`] (method name bytes plus a flattened
//! [`Param`] list, mirroring upstream's `def_node.arguments.reject(&
//! :blockarg_type?)`) on `enter`/`leave`, reading `self.def_stack.last()`
//! from a [`NodeKind::CallNode`] visit to reproduce "nearest enclosing
//! `def`/`defs`" without a separate ancestor walk. Prism's grammar only
//! ever orders a method's parameters `requireds*, optionals*, rest?,
//! posts*, keywords*, keyword_rest?, block?`
//! ([`ruby_ast::node::ParametersNode`]'s own field order), so
//! concatenating those buckets (minus `block`) reproduces upstream's
//! left-to-right `node.arguments` order exactly.
//!
//! `...` (argument forwarding) occupies `ParametersNode::keyword_rest()`
//! as a [`NodeKind::ForwardingParameterNode`] (verified against a real
//! Prism parse of `def m(...)`), the same slot a real `**kwrest` would
//! use; [`Param::Forward`] is read off that slot rather than off `rest()`.
//! A braceless keyword-argument hash at a call site (`x: x`) is Prism's
//! [`NodeKind::KeywordHashNode`]; a braced hash literal passed in the same
//! position (`{ x: x }`) is instead [`NodeKind::HashNode`] -- two distinct
//! kinds where whitequark has one `hash` type distinguished by `braces?`.
//! [`keyword_hash_elements`] (upstream's `keyword_hash_argument?`) matches
//! only `KeywordHashNode`, reproducing `hash_type? && !braces?`; the
//! `extra_keyword_arguments?` path (upstream's `passed_keyword_arguments`/
//! `expandable_keyword_arguments?`, which read `hash_type?` without the
//! brace restriction) instead uses [`hash_like_elements`], matching either
//! kind. A bare `*`/`**` used as a call argument (anonymous splat
//! forwarding) is the same [`NodeKind::SplatNode`]/[`NodeKind::
//! AssocSplatNode`] Prism gives a named `*args`/`**kwargs` splat, just
//! with `expression`/`value` absent, so comparing the whole argument's
//! source text against the whole parameter's source text (as upstream's
//! `send_arg.source == def_arg.source` does) handles the anonymous and
//! named forms identically with no extra casework.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{DefNode, ParametersNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Upstream's `MSG`.
const MSG: &str = "Ensure you correctly provided all the arguments.";

/// One flattened, non-block parameter of the enclosing `def`/`defs`,
/// reduced to just what `argument_match?`/the arity checks need. Owned so
/// it can outlive the `enter(DefNode)` call that built it, per [`Rule`]'s
/// `'static` bound.
#[derive(Debug, Clone)]
enum Param {
    /// `:arg` or `:restarg`: matched against a call argument's own source
    /// text (upstream's `send_arg.source == def_arg.source`).
    Positional { rest: bool, source: Vec<u8> },
    /// `:optarg`: matched against a call argument's source text equalling
    /// just the parameter's own name (upstream's `send_arg.source ==
    /// def_arg_name.to_s`).
    Optional(Vec<u8>),
    /// `:kwarg`/`:kwoptarg`: matched by name against a braceless keyword
    /// hash argument's pairs.
    Keyword(Vec<u8>),
    /// `:kwrestarg`: matched against a braceless keyword hash argument's
    /// `**`-splat child's source text.
    KeywordRest(Vec<u8>),
    /// `:forward_arg` (`...`): matched against a call argument being
    /// itself a `...` forward.
    Forward,
}

/// The enclosing `def`/`defs`'s own name and flattened parameter list.
#[derive(Debug, Clone)]
struct DefFrame {
    name: Vec<u8>,
    params: Vec<Param>,
}

/// Ensures that `to_enum`/`enum_for`, called for the current method, has correct arguments.
#[derive(Debug, Clone, Default)]
pub struct ToEnumArguments {
    def_stack: Vec<DefFrame>,
}

impl Rule for ToEnumArguments {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ToEnumArguments",
        department: Department::Lint,
        summary: "Ensures that `to_enum`/`enum_for`, called for the current method, has correct arguments.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode => {
                let def = node.as_def_node().expect("kind matched");
                self.def_stack.push(build_def_frame(&def, ctx));
            }
            NodeKind::CallNode => self.check_call(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::DefNode {
            self.def_stack.pop();
        }
    }
}

impl ToEnumArguments {
    /// Upstream's `on_send`.
    fn check_call(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(frame) = self.def_stack.last() else { return };
        let call = node.as_call_node().expect("kind matched");

        if call.receiver().is_some() && call.receiver().and_then(|r| r.as_self_node()).is_none() {
            return;
        }
        if !matches!(call.name().as_slice(), b"to_enum" | b"enum_for") {
            return;
        }

        let Some(all_args) = call.arguments() else { return };
        let all_args = all_args.arguments();
        let Some(method_node) = all_args.iter().next() else { return };
        let rest: Vec<Node<'_>> = all_args.iter().skip(1).collect();

        if !is_method_name(&method_node, &frame.name) {
            return;
        }
        if arguments_match(ctx, &rest, frame) {
            return;
        }

        ctx.report(&Self::META, node.span(), MSG);
    }
}

/// Upstream's `enum_conversion_call?`'s `method_name?` matcher.
fn is_method_name(method_node: &Node<'_>, def_name: &[u8]) -> bool {
    if let Some(call) = method_node.as_call_node() {
        return call.receiver().is_none()
            && call.arguments().is_none()
            && matches!(call.name().as_slice(), b"__method__" | b"__callee__");
    }
    method_node.as_symbol_node().is_some_and(|sym| sym.unescaped() == def_name)
}

/// Upstream's `arguments_match?`.
fn arguments_match(ctx: &Context<'_>, call_args: &[Node<'_>], frame: &DefFrame) -> bool {
    let mut index = 0usize;
    let all_present = frame.params.iter().all(|param| {
        let is_positional = matches!(param, Param::Positional { .. } | Param::Optional(_));
        let send_arg = call_args.get(index).copied();
        if is_positional {
            index += 1;
        }
        send_arg.is_some_and(|arg| argument_match(ctx, &arg, param))
    });

    all_present
        && !extra_positional_arguments(call_args, frame)
        && !extra_keyword_arguments(call_args, frame)
}

/// Upstream's `argument_match?`.
fn argument_match(ctx: &Context<'_>, send_arg: &Node<'_>, param: &Param) -> bool {
    match param {
        Param::Positional { source, .. } => ctx.text(send_arg.span()) == source.as_slice(),
        Param::Optional(name) => ctx.text(send_arg.span()) == name.as_slice(),
        Param::Keyword(name) => keyword_hash_elements(send_arg)
            .is_some_and(|elements| elements.iter().any(|el| passes_keyword_arg(el, name))),
        Param::KeywordRest(source) => keyword_hash_elements(send_arg).is_some_and(|elements| {
            elements.iter().any(|el| {
                el.as_assoc_splat_node().is_some() && ctx.text(el.span()) == source.as_slice()
            })
        }),
        Param::Forward => send_arg.as_forwarding_arguments_node().is_some(),
    }
}

/// Upstream's `passing_keyword_arg?`: `(pair (sym %1) (lvar %1))`. A
/// value-omission shorthand pair (`file:`, short for `file: file`) wraps
/// its value in Prism's [`NodeKind::ImplicitNode`] rather than giving a
/// bare `lvar` directly -- whitequark's parser desugars `{file:}` into the
/// same explicit `(pair (sym :file) (lvar :file))` at parse time, so this
/// unwraps one `ImplicitNode` level to match that normalization.
fn passes_keyword_arg(element: &Node<'_>, name: &[u8]) -> bool {
    let Some(assoc) = element.as_assoc_node() else { return false };
    if assoc.key().as_symbol_node().is_none_or(|sym| sym.unescaped() != name) {
        return false;
    }
    let value = assoc.value();
    let value = value.as_implicit_node().map_or(value, |implicit| implicit.value());
    value.as_local_variable_read_node().is_some_and(|lvar| lvar.name().as_slice() == name)
}

/// Upstream's `extra_positional_arguments?`.
fn extra_positional_arguments(call_args: &[Node<'_>], frame: &DefFrame) -> bool {
    let variadic = frame
        .params
        .iter()
        .any(|p| matches!(p, Param::Positional { rest: true, .. } | Param::Forward));
    if variadic || is_expandable(call_args) {
        return false;
    }

    let positional_params = frame
        .params
        .iter()
        .filter(|p| matches!(p, Param::Positional { rest: false, .. } | Param::Optional(_)))
        .count();
    let positional_args = call_args
        .iter()
        .filter(|arg| hash_like_elements(arg).is_none() && arg.as_block_argument_node().is_none())
        .count();

    positional_args > positional_params
}

/// Upstream's `expandable_arguments?`.
fn is_expandable(call_args: &[Node<'_>]) -> bool {
    call_args
        .iter()
        .any(|arg| arg.as_splat_node().is_some() || arg.as_forwarding_arguments_node().is_some())
}

/// Upstream's `extra_keyword_arguments?`.
fn extra_keyword_arguments(call_args: &[Node<'_>], frame: &DefFrame) -> bool {
    let variadic = frame.params.iter().any(|p| matches!(p, Param::KeywordRest(_) | Param::Forward));
    if variadic || is_expandable_keyword(call_args) {
        return false;
    }

    let declared: Vec<&[u8]> = frame
        .params
        .iter()
        .filter_map(|p| if let Param::Keyword(name) = p { Some(name.as_slice()) } else { None })
        .collect();

    passed_keyword_names(call_args).iter().any(|name| !declared.contains(&name.as_slice()))
}

/// Upstream's `expandable_keyword_arguments?`.
fn is_expandable_keyword(call_args: &[Node<'_>]) -> bool {
    call_args.iter().any(|arg| {
        arg.as_forwarding_arguments_node().is_some()
            || hash_like_elements(arg).is_some_and(|elements| {
                elements.iter().any(|el| el.as_assoc_splat_node().is_some())
            })
    })
}

/// Upstream's `passed_keyword_arguments`.
fn passed_keyword_names(call_args: &[Node<'_>]) -> Vec<Vec<u8>> {
    call_args
        .iter()
        .filter_map(hash_like_elements)
        .flatten()
        .filter_map(|el| {
            let assoc = el.as_assoc_node()?;
            let sym = assoc.key().as_symbol_node()?;
            Some(sym.unescaped().to_vec())
        })
        .collect()
}

/// Upstream's `keyword_hash_argument?`: a braceless keyword hash
/// (`KeywordHashNode`), never a braced hash literal (`HashNode`).
fn keyword_hash_elements<'a>(node: &Node<'a>) -> Option<Vec<Node<'a>>> {
    node.as_keyword_hash_node().map(|h| h.elements().iter().collect())
}

/// A call argument's elements regardless of whether it is a braced hash
/// literal or a braceless keyword hash (upstream's bare `hash_type?`,
/// used where brace-ness doesn't matter).
fn hash_like_elements<'a>(node: &Node<'a>) -> Option<Vec<Node<'a>>> {
    if let Some(h) = node.as_hash_node() {
        return Some(h.elements().iter().collect());
    }
    node.as_keyword_hash_node().map(|h| h.elements().iter().collect())
}

/// Upstream's `def_node.arguments.reject(&:blockarg_type?)`, flattened in
/// Prism's own `requireds, optionals, rest, posts, keywords, keyword_rest`
/// field order (which always reproduces source order -- see the module
/// doc).
fn build_def_frame(def: &DefNode<'_>, ctx: &Context<'_>) -> DefFrame {
    let mut params = Vec::new();
    let Some(parameters) = def.parameters() else {
        return DefFrame { name: def.name().as_slice().to_vec(), params };
    };

    push_positional_params(&parameters, ctx, &mut params);

    for keyword in &parameters.keywords() {
        params.push(Param::Keyword(keyword_param_name(&keyword)));
    }

    if let Some(keyword_rest) = parameters.keyword_rest() {
        if keyword_rest.as_forwarding_parameter_node().is_some() {
            params.push(Param::Forward);
        } else {
            params.push(Param::KeywordRest(ctx.text(keyword_rest.span()).to_vec()));
        }
    }

    DefFrame { name: def.name().as_slice().to_vec(), params }
}

/// The `requireds, optionals, rest, posts` portion of [`build_def_frame`].
fn push_positional_params(
    parameters: &ParametersNode<'_>,
    ctx: &Context<'_>,
    params: &mut Vec<Param>,
) {
    for required in &parameters.requireds() {
        params.push(Param::Positional { rest: false, source: ctx.text(required.span()).to_vec() });
    }
    for optional in &parameters.optionals() {
        let optional =
            optional.as_optional_parameter_node().expect("optionals are OptionalParameterNode");
        params.push(Param::Optional(optional.name().as_slice().to_vec()));
    }
    if let Some(rest) = parameters.rest() {
        params.push(Param::Positional { rest: true, source: ctx.text(rest.span()).to_vec() });
    }
    for post in &parameters.posts() {
        params.push(Param::Positional { rest: false, source: ctx.text(post.span()).to_vec() });
    }
}

/// A `keywords` entry's own name, regardless of whether it is required or has a default.
fn keyword_param_name(node: &Node<'_>) -> Vec<u8> {
    if let Some(k) = node.as_required_keyword_parameter_node() {
        return k.name().as_slice().to_vec();
    }
    node.as_optional_keyword_parameter_node()
        .expect("keywords are keyword parameter nodes")
        .name()
        .as_slice()
        .to_vec()
}

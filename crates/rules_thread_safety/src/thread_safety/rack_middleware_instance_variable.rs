//! `ThreadSafety/RackMiddlewareInstanceVariable`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/rack_middleware_instance_variable.rb`.

use std::collections::HashSet;

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::{CallNode, DefNode, ParametersNode};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Application variable name plus the names of variables holding thread-safe objects.
type Variables = (Vec<u8>, Vec<Vec<u8>>);

const MSG: &str = "Avoid instance variables in Rack middleware.";

/// Avoid instance variables in Rack middleware.
#[derive(Debug, Clone)]
pub struct RackMiddlewareInstanceVariable {
    allowed_identifiers: Vec<String>,
    reported: HashSet<(u32, u32)>,
}

impl Rule for RackMiddlewareInstanceVariable {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/RackMiddlewareInstanceVariable",
        department: Department::ThreadSafety,
        summary: "Avoid instance variables in Rack middleware.",
        explanation: "Middlewares are initialized once, meaning any instance variables are shared between executor threads. To avoid potential race conditions, design middlewares to be stateless or implement proper synchronization.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ClassNode, NodeKind::CallNode],
        config: &[ConfigOption {
            name: "AllowedIdentifiers",
            default: ConfigDefault::StrList(&[]),
            allowed: &[],
            doc: "Instance variable names (sigils stripped) that are never reported.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allowed_identifiers: options.str_list("AllowedIdentifiers"),
            reported: HashSet::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ClassNode => self.on_class(node, ctx),
            NodeKind::CallNode => self.on_send(node, ctx),
            _ => {}
        }
    }
}

impl RackMiddlewareInstanceVariable {
    /// `AllowedIdentifiers#allowed_identifier?`.
    fn allowed_identifier(&self, name: &[u8]) -> bool {
        if self.allowed_identifiers.is_empty() {
            return false;
        }
        let stripped: String =
            String::from_utf8_lossy(name).chars().filter(|c| !matches!(c, '@' | '$')).collect();
        self.allowed_identifiers.contains(&stripped)
    }

    fn add_offense(&mut self, ctx: &mut Context<'_>, span: Span) {
        if self.reported.insert((span.start, span.end)) {
            ctx.report(&Self::META, span, MSG);
        }
    }

    fn on_class(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !rack_middleware_like_class(node) {
            return;
        }

        // `find_constructor_method` + the two extraction helpers.
        let mut constructor: Option<Option<Variables>> = None;
        each_descendant(node, &mut |d| {
            if constructor.is_some() {
                return;
            }
            if let Some(def) = d.as_def_node() {
                if def.name().as_slice() == b"initialize" && param_count(&def) >= 1 {
                    constructor = Some(extract_variables(d));
                }
            }
        });
        let Some(Some((application_variable, safe_variables))) = constructor else { return };

        let mut found: Vec<Span> = Vec::new();
        let mut inner = |name: &[u8], span: Span| {
            if name == application_variable.as_slice()
                || safe_variables.iter().any(|v| v == name)
                || self.allowed_identifier(name)
            {
                return;
            }
            found.push(span);
        };
        each_descendant(node, &mut |d| {
            let Some(def) = d.as_def_node() else { return };
            if def.receiver().is_some() {
                return;
            }
            each_descendant(d, &mut |n| {
                if let Some((name, span)) = ivar_node(n) {
                    inner(name, span);
                }
            });
        });
        for span in found {
            self.add_offense(ctx, span);
        }
    }

    fn on_send(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        if name.as_slice() != b"instance_variable_get" && name.as_slice() != b"instance_variable_set"
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(argument) = arguments.arguments().iter().next() else { return };
        let value: Vec<u8> = if let Some(sym) = argument.as_symbol_node() {
            sym.unescaped().to_vec()
        } else if let Some(string) = argument.as_string_node() {
            string.unescaped().to_vec()
        } else {
            return;
        };
        if self.allowed_identifier(&value) {
            return;
        }
        let mut span = call_span_excluding_block(&call);
        if let Some(block) = call.block() {
            if block.kind() == NodeKind::BlockArgumentNode {
                span.end = span.end.max(block.span().end);
            }
        }
        self.add_offense(ctx, span);
    }
}

/// The instance variable name and offense range of an `ivasgn`/`ivar` node.
fn ivar_node<'a>(node: &'a Node<'_>) -> Option<(&'a [u8], Span)> {
    match node.kind() {
        NodeKind::InstanceVariableReadNode => {
            let n = node.as_instance_variable_read_node()?;
            Some((n.name().as_slice(), node.span()))
        }
        NodeKind::InstanceVariableTargetNode => {
            let n = node.as_instance_variable_target_node()?;
            Some((n.name().as_slice(), node.span()))
        }
        NodeKind::InstanceVariableWriteNode => {
            let n = node.as_instance_variable_write_node()?;
            Some((n.name().as_slice(), node.span()))
        }
        NodeKind::InstanceVariableOperatorWriteNode => {
            let n = node.as_instance_variable_operator_write_node()?;
            Some((n.name().as_slice(), n.name_loc().span()))
        }
        NodeKind::InstanceVariableOrWriteNode => {
            let n = node.as_instance_variable_or_write_node()?;
            Some((n.name().as_slice(), n.name_loc().span()))
        }
        NodeKind::InstanceVariableAndWriteNode => {
            let n = node.as_instance_variable_and_write_node()?;
            Some((n.name().as_slice(), n.name_loc().span()))
        }
        _ => None,
    }
}

/// `arguments.size` of a `def`: every parameter.
fn param_count(def: &DefNode<'_>) -> usize {
    def.parameters().map_or(0, |p| {
        p.requireds().iter().count()
            + p.optionals().iter().count()
            + usize::from(p.rest().is_some())
            + p.posts().iter().count()
            + p.keywords().iter().count()
            + usize::from(p.keyword_rest().is_some())
            + usize::from(p.block().is_some())
    })
}

/// Only plain required parameters (`(args (arg _)+)` / `(args (arg _))`),
/// `min..=max` of them.
fn only_required(params: Option<ParametersNode<'_>>, exactly_one: bool) -> bool {
    let Some(p) = params else { return false };
    if p.optionals().iter().next().is_some()
        || p.rest().is_some()
        || p.posts().iter().next().is_some()
        || p.keywords().iter().next().is_some()
        || p.keyword_rest().is_some()
        || p.block().is_some()
    {
        return false;
    }
    let mut count = 0;
    for r in &p.requireds() {
        if r.kind() != NodeKind::RequiredParameterNode {
            return false;
        }
        count += 1;
    }
    if exactly_one {
        count == 1
    } else {
        count >= 1
    }
}

/// `rack_middleware_like_class?`.
fn rack_middleware_like_class(node: &Node<'_>) -> bool {
    let Some(class) = node.as_class_node() else { return false };
    if class.constant_path().kind() != NodeKind::ConstantReadNode || class.superclass().is_some() {
        return false;
    }
    let Some(body) = class.body() else { return false };
    let Some(statements) = body.as_statements_node() else { return false };
    if statements.body().iter().count() < 2 {
        return false;
    }
    let mut has_initialize = false;
    let mut has_call = false;
    for stmt in &statements.body() {
        let Some(def) = stmt.as_def_node() else { continue };
        if def.receiver().is_some() {
            continue;
        }
        match def.name().as_slice() {
            b"initialize" if only_required(def.parameters(), false) => has_initialize = true,
            b"call" if only_required(def.parameters(), true) => has_call = true,
            _ => {}
        }
    }
    has_initialize && has_call
}

/// `app_variable` + `extract_safe_variables_from_constructor_method`.
fn extract_variables(constructor: &Node<'_>) -> Option<Variables> {
    let mut application: Option<Vec<u8>> = None;
    let mut check = |def_node: &Node<'_>| {
        if application.is_some() {
            return;
        }
        let Some(def) = def_node.as_def_node() else { return };
        if def.name().as_slice() != b"initialize" || def.receiver().is_some() {
            return;
        }
        let first_is_arg = def
            .parameters()
            .and_then(|p| p.requireds().iter().next())
            .is_some_and(|r| r.kind() == NodeKind::RequiredParameterNode);
        if !first_is_arg {
            return;
        }
        each_descendant(def_node, &mut |n| {
            if application.is_some() {
                return;
            }
            if let Some(write) = n.as_instance_variable_write_node() {
                if write.value().kind() == NodeKind::LocalVariableReadNode {
                    application = Some(write.name().as_slice().to_vec());
                }
            }
        });
    };
    check(constructor);
    each_descendant(constructor, &mut |d| check(d));
    let application = application?;

    let mut safe: Vec<Vec<u8>> = Vec::new();
    let mut collect = |n: &Node<'_>| {
        if let Some(write) = n.as_instance_variable_write_node() {
            if operation_produces_threadsafe_object(&write.value()) {
                safe.push(write.name().as_slice().to_vec());
            }
        }
    };
    collect(constructor);
    each_descendant(constructor, &mut |n| collect(n));
    Some((application, safe))
}

/// The names of a constant path rooted at nothing or `::`; `None` when the
/// root is any other expression.
fn const_chain(node: &Node<'_>) -> Option<Vec<Vec<u8>>> {
    if let Some(c) = node.as_constant_read_node() {
        return Some(vec![c.name().as_slice().to_vec()]);
    }
    let path = node.as_constant_path_node()?;
    let name = path.name()?.as_slice().to_vec();
    let mut chain = match path.parent() {
        Some(parent) => const_chain(&parent)?,
        None => Vec::new(),
    };
    chain.push(name);
    Some(chain)
}

fn is_threadsafe_new(call: &CallNode<'_>) -> bool {
    if call.is_safe_navigation() || call.name().as_slice() != b"new" {
        return false;
    }
    // `numblock`/`itblock` are not `block`.
    if let Some(block) = call.block().as_ref().and_then(Node::as_block_node) {
        if block.parameters().is_some_and(|p| {
            matches!(p.kind(), NodeKind::NumberedParametersNode | NodeKind::ItParametersNode)
        }) {
            return false;
        }
    }
    let Some(receiver) = call.receiver() else { return false };
    let Some(chain) = const_chain(&receiver) else { return false };
    let names: Vec<&[u8]> = chain.iter().map(Vec::as_slice).collect();
    matches!(
        names.as_slice(),
        [b"Queue"]
            | [b"ThreadSafe", b"Hash" | b"Array"]
            | [b"Concurrent", _]
            | [b"Concurrent", _, _]
            | [b"Concurrent", _, _, _]
    )
}

/// `OperationWithThreadsafeResult#operation_produces_threadsafe_object?`.
fn operation_produces_threadsafe_object(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| is_threadsafe_new(&call))
}

//! `Style/MapIntoArray`, ported from RuboCop's
//! `lib/rubocop/cop/style/map_into_array.rb`.
//!
//! RuboCop hooks `VariableForce` (`after_leaving_scope`) to find the local
//! variable a `<<`/`push`/`append` call targets; here that is
//! [`ruby_semantic::Semantics`], which already resolves each read to the
//! right variable (respecting shadowing), so `find_dest_var` collapses to a
//! linear search for the variable whose references contain the receiver
//! node. Prism has no parent pointers, so a `(kind, span) -> parent` map is
//! built once from `file_start` for everything upstream reaches through
//! `#parent`/`#right_sibling`/`#declaration_node` (RuboCop's
//! `each_block_with_push?` top-level `^({begin kwbegin block} ...)` guard
//! becomes "the call's parent is a `StatementsNode`", since Prism always
//! wraps a block/def/program body in one, even for a single statement --
//! whitequark's `begin` elision has no Prism counterpart).

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode, DefNode};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_semantic::{same, AssignmentId, DeclKind, ScopeKind, Semantics, VariableId};
use ruby_source::{Side, Span};

/// RuboCop's `MSG` format string.
const MSG_FMT: &str = "Use `{}` instead of `each` to map elements into an array.";

/// Checks for usages of `each` with `<<`, `push`, or `append` which can be replaced by `map`.
#[derive(Debug, Clone)]
pub struct MapIntoArray {
    new_method_name: String,
}

impl Rule for MapIntoArray {
    const META: RuleMeta = RuleMeta {
        name: "Style/MapIntoArray",
        department: Department::Style,
        summary: "Checks for usages of `each` with `<<`, `push`, or `append` which can be replaced by `map`.",
        explanation: "\
If `PreferredMethods` is configured for `map` in `Style/CollectionMethods`, this cop uses the \
specified method for replacement.

NOTE: The return value of `Enumerable#each` is `self`, whereas the return value of \
`Enumerable#map` is an `Array`. They are not autocorrected when a return value could be used \
because these types differ.

NOTE: It only detects when the mapping destination is either: a local variable initialized as an \
empty array and referred to only by the pushing operation; or, if it is the single block argument \
to a `[].tap` block. This is because, if not, it's challenging to statically guarantee that the \
mapping destination variable remains an empty array.

@safety
  This cop is unsafe because not all objects that have an `each` method also have a `map` method \
(e.g. `ENV`). Additionally, for calls with a block, not all objects that have a `map` method return \
an array (e.g. `Enumerator::Lazy`).",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let new_method_name = options
            .peer("Style/CollectionMethods", "PreferredMethods")
            .and_then(OptionValue::as_map)
            .and_then(|entries| entries.iter().find(|(name, _)| name == "map"))
            .and_then(|(_, value)| value.as_str())
            .map_or_else(|| "map".to_string(), ToString::to_string);
        Ok(Self { new_method_name })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let analysis = Analysis::build(root);

        let mut offenses: Vec<(Span, String, Option<Fix>)> = Vec::new();
        {
            let semantics = ctx.semantics();
            for candidate in &analysis.candidates {
                if let Some(offense) =
                    inspect(candidate, semantics, &analysis, ctx, &self.new_method_name)
                {
                    offenses.push(offense);
                }
            }
        }

        for (span, message, fix) in offenses {
            match fix {
                Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
                None => ctx.report(&Self::META, span, message),
            }
        }
    }
}

/// One `src.each { |e| dest << ... }` candidate: RuboCop's `each_block_with_push?`.
struct Candidate<'pr> {
    call: CallNode<'pr>,
    push: CallNode<'pr>,
    arg: Node<'pr>,
}

/// `(kind, span) -> parent` map plus every `each_block_with_push?` match in the file.
struct Analysis<'pr> {
    parent: HashMap<(NodeKind, Span), Node<'pr>>,
    candidates: Vec<Candidate<'pr>>,
}

impl<'pr> Analysis<'pr> {
    fn build(root: Node<'pr>) -> Self {
        let mut analysis = Self { parent: HashMap::new(), candidates: Vec::new() };
        analysis.visit(root);
        analysis
    }

    fn visit(&mut self, node: Node<'pr>) {
        if let Some(candidate) = match_candidate(&node, &self.parent) {
            self.candidates.push(candidate);
        }
        let mut children = Vec::new();
        for_each_child(&node, |child| children.push(*child));
        for child in children {
            self.parent.insert((child.kind(), child.span()), node);
            self.visit(child);
        }
    }

    fn parent_of(&self, node: &Node<'pr>) -> Option<Node<'pr>> {
        self.parent.get(&(node.kind(), node.span())).copied()
    }
}

/// RuboCop's `each_block_with_push?`, including the top-level `^({begin kwbegin block} ...)`
/// guard (here: the call's parent must already be a `StatementsNode`, since Prism never elides
/// the wrapper for a single statement the way whitequark does).
fn match_candidate<'pr>(
    node: &Node<'pr>,
    parent: &HashMap<(NodeKind, Span), Node<'pr>>,
) -> Option<Candidate<'pr>> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"each" {
        return None;
    }
    let receiver = call.receiver()?;
    if matches!(receiver.kind(), NodeKind::NilNode | NodeKind::SelfNode) {
        return None;
    }
    if call.arguments().is_some() {
        return None;
    }
    parent.get(&(node.kind(), node.span()))?.as_statements_node()?;

    let block = call.block()?.as_block_node()?;
    let body = block.body()?;
    let stmts = body.as_statements_node()?;
    let items: Vec<Node<'pr>> = stmts.body().iter().collect();
    let [only] = items.as_slice() else { return None };
    let push = only.as_call_node()?;
    if push.block().is_some() {
        return None;
    }
    if !matches!(push.name().as_slice(), b"<<" | b"push" | b"append") {
        return None;
    }
    push.receiver()?.as_local_variable_read_node()?;
    let args = push.arguments()?;
    let arg_list: Vec<Node<'pr>> = args.arguments().iter().collect();
    let [arg] = arg_list.as_slice() else { return None };
    if !suitable_argument(arg) {
        return None;
    }

    Some(Candidate { call, push, arg: *arg })
}

/// RuboCop's `suitable_argument_node?`.
fn suitable_argument(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::SplatNode | NodeKind::ForwardingArgumentsNode | NodeKind::BlockArgumentNode => {
            false
        }
        NodeKind::KeywordHashNode => {
            let hash = node.as_keyword_hash_node().expect("kind matched");
            let elements: Vec<Node<'_>> = hash.elements().iter().collect();
            !matches!(elements.as_slice(), [only]
                if only.as_assoc_splat_node().is_some_and(|splat| splat.value().is_none()))
        }
        _ => true,
    }
}

/// RuboCop's `on_block` plus `register_offense`.
fn inspect<'pr>(
    candidate: &Candidate<'pr>,
    semantics: &Semantics<'pr>,
    analysis: &Analysis<'pr>,
    ctx: &Context<'_>,
    new_method_name: &str,
) -> Option<(Span, String, Option<Fix>)> {
    let push_receiver = candidate.push.receiver()?;
    let dest_name = push_receiver.as_local_variable_read_node()?.name().as_slice();
    let dest_var = find_dest_var(semantics, dest_name, &push_receiver)?;

    let call_span = candidate.call.as_node().span();
    let tap_block = offending_empty_array_tap(semantics, analysis, dest_var, &candidate.call);

    let asgn = if tap_block.is_some() {
        None
    } else {
        let asgn = find_closest_assignment(semantics, dest_var, call_span)?;
        let asgn_node = semantics.assignment(asgn).node();
        let write = asgn_node.as_local_variable_write_node()?;
        if !is_empty_array_value(&write.value()) {
            return None;
        }
        if !dest_used_only_for_mapping(semantics, analysis, dest_var, asgn, &candidate.call) {
            return None;
        }
        Some(asgn)
    };

    register_offense(
        candidate,
        semantics,
        analysis,
        ctx,
        dest_var,
        asgn,
        tap_block,
        new_method_name,
    )
}

/// RuboCop's `find_dest_var`: the variable whose references include this exact receiver node.
fn find_dest_var<'pr>(
    semantics: &Semantics<'pr>,
    name: &[u8],
    dest_read: &Node<'pr>,
) -> Option<VariableId> {
    semantics.variable_ids().find(|&id| {
        let var = semantics.variable(id);
        var.name() == name && var.references().iter().any(|r| same(&r.node(), dest_read))
    })
}

/// RuboCop's `offending_empty_array_tap?` plus `empty_array_tap`: `dest`'s declaration is the
/// sole block parameter of a `[].tap do |dest| ... end` whose body is exactly this `each` call.
fn offending_empty_array_tap<'pr>(
    semantics: &Semantics<'pr>,
    analysis: &Analysis<'pr>,
    dest_var: VariableId,
    call: &CallNode<'pr>,
) -> Option<CallNode<'pr>> {
    let var = semantics.variable(dest_var);
    if var.decl_kind() != DeclKind::RequiredArg {
        return None;
    }
    let scope = semantics.scope(var.scope());
    if scope.kind() != ScopeKind::Block {
        return None;
    }
    let block_node = scope.node().as_block_node()?;
    let owner_call = analysis.parent_of(&scope.node())?;
    let owner_call = owner_call.as_call_node()?;
    if owner_call.name().as_slice() != b"tap" {
        return None;
    }
    if !owner_call.receiver()?.as_array_node().is_some_and(|arr| arr.elements().is_empty()) {
        return None;
    }
    single_required_param(&block_node)?;

    let body = block_node.body()?;
    let stmts = body.as_statements_node()?;
    let items: Vec<Node<'pr>> = stmts.body().iter().collect();
    match items.as_slice() {
        [only] if same(only, &call.as_node()) => Some(owner_call),
        _ => None,
    }
}

/// RuboCop's `(args (arg _))` shape check (exactly one required parameter, nothing else).
fn single_required_param(block: &BlockNode<'_>) -> Option<()> {
    let params = block.parameters()?;
    let params = params.as_block_parameters_node()?;
    if !params.locals().is_empty() {
        return None;
    }
    let inner = params.parameters()?;
    if inner.requireds().len() != 1
        || !inner.optionals().is_empty()
        || inner.rest().is_some()
        || !inner.posts().is_empty()
        || !inner.keywords().is_empty()
        || inner.keyword_rest().is_some()
        || inner.block().is_some()
    {
        return None;
    }
    Some(())
}

/// RuboCop's `find_closest_assignment`.
fn find_closest_assignment(
    semantics: &Semantics<'_>,
    dest_var: VariableId,
    before: Span,
) -> Option<AssignmentId> {
    semantics
        .variable(dest_var)
        .assignments()
        .iter()
        .rev()
        .find(|&&id| semantics.assignment(id).node().span().end < before.start)
        .copied()
}

/// RuboCop's `empty_array_asgn?`.
fn is_empty_array_value(value: &Node<'_>) -> bool {
    if let Some(array) = value.as_array_node() {
        return array.elements().is_empty();
    }
    let Some(call) = value.as_call_node() else { return false };
    match call.name().as_slice() {
        b"[]" => is_array_const(call.receiver().as_ref()) && call.arguments().is_none(),
        b"new" => is_array_const(call.receiver().as_ref()) && is_empty_array_arg(&call, true),
        b"Array" => call.receiver().is_none() && is_empty_array_arg(&call, false),
        _ => false,
    }
}

fn is_array_const(receiver: Option<&Node<'_>>) -> bool {
    receiver.and_then(ruby_ast::ext::const_name).as_deref() == Some("Array")
}

/// One argument that is an empty array literal; `optional` allows zero arguments too.
fn is_empty_array_arg(call: &CallNode<'_>, optional: bool) -> bool {
    let Some(args) = call.arguments() else { return optional };
    let list: Vec<Node<'_>> = args.arguments().iter().collect();
    matches!(list.as_slice(), [only] if only.as_array_node().is_some_and(|a| a.elements().is_empty()))
}

/// RuboCop's `dest_used_only_for_mapping?`.
fn dest_used_only_for_mapping(
    semantics: &Semantics<'_>,
    analysis: &Analysis<'_>,
    dest_var: VariableId,
    asgn: AssignmentId,
    call: &CallNode<'_>,
) -> bool {
    let asgn_node = semantics.assignment(asgn).node();
    let Some(asgn_parent) = analysis.parent_of(&asgn_node) else { return false };
    let Some(call_parent) = analysis.parent_of(&call.as_node()) else { return false };
    if !same(&asgn_parent, &call_parent) {
        return false;
    }

    let range = Span::new(asgn_node.span().start, call.as_node().span().end);
    let var = semantics.variable(dest_var);
    let refs_in_range =
        var.references().iter().filter(|r| range_contains(range, r.node().span())).count();
    if refs_in_range != 1 {
        return false;
    }
    let asgns_in_range = var
        .assignments()
        .iter()
        .filter(|&&id| range_contains(range, semantics.assignment(id).node().span()))
        .count();
    asgns_in_range == 1
}

fn range_contains(range: Span, span: Span) -> bool {
    range.start <= span.start && span.end <= range.end
}

/// RuboCop's `register_offense` plus the `corrector` block.
#[allow(clippy::too_many_arguments)]
fn register_offense<'pr>(
    candidate: &Candidate<'pr>,
    semantics: &Semantics<'pr>,
    analysis: &Analysis<'pr>,
    ctx: &Context<'_>,
    dest_var: VariableId,
    asgn: Option<AssignmentId>,
    tap_block: Option<CallNode<'pr>>,
    new_method_name: &str,
) -> Option<(Span, String, Option<Fix>)> {
    let message = MSG_FMT.replacen("{}", new_method_name, 1);
    let span = candidate.call.as_node().span();

    if return_value_used(candidate.call.as_node(), analysis) {
        return Some((span, message, None));
    }

    let mut edits = Vec::new();
    let selector = candidate.call.message_loc()?.span();
    edits.push(Edit::replace(selector, new_method_name.as_bytes().to_vec()));

    if let Some(owner_call) = tap_block {
        let block = owner_call.block()?.as_block_node()?;
        let prefix = Span::new(owner_call.as_node().span().start, span.start);
        edits.push(Edit::delete(prefix));
        let closing = block.closing_loc().span();
        let trailing = ctx.with_surrounding_space(closing, Side::Left, true, false);
        edits.push(Edit::delete(trailing));
    } else {
        let asgn_span = semantics.assignment(asgn?).node().span();
        let r1 = ctx.with_surrounding_space(asgn_span, Side::Right, true, false);
        let r2 = ctx.with_surrounding_space(r1, Side::Right, false, false);
        edits.push(Edit::delete(r2));
    }

    correct_push_node(candidate, &mut edits);
    correct_return_value_handling(candidate, semantics, analysis, ctx, dest_var, &mut edits);

    Some((span, message, Some(Fix { applicability: Applicability::Unsafe, edits })))
}

/// RuboCop's `correct_push_node`.
fn correct_push_node(candidate: &Candidate<'_>, edits: &mut Vec<Edit>) {
    let range = candidate.push.as_node().span();
    let arg_range = candidate.arg.span();

    if candidate.arg.kind() == NodeKind::KeywordHashNode {
        edits.push(Edit::insert(arg_range.start, b"{ ".to_vec()));
        edits.push(Edit::insert(arg_range.end, b" }".to_vec()));
    }
    edits.push(Edit::delete(Span::new(range.start, arg_range.start)));
    edits.push(Edit::delete(Span::new(arg_range.end, range.end)));
}

/// RuboCop's `correct_return_value_handling`.
fn correct_return_value_handling<'pr>(
    candidate: &Candidate<'pr>,
    semantics: &Semantics<'pr>,
    analysis: &Analysis<'pr>,
    ctx: &Context<'_>,
    dest_var: VariableId,
    edits: &mut Vec<Edit>,
) {
    let name = semantics.variable(dest_var).name();
    if let Some(next) = right_sibling(&candidate.call.as_node(), analysis) {
        if next.as_local_variable_read_node().is_some_and(|r| r.name().as_slice() == name) {
            let doomed = ctx.with_surrounding_space(next.span(), Side::Left, true, false);
            edits.push(Edit::delete(doomed));
        }
    }
    let mut text = name.to_vec();
    text.extend_from_slice(b" = ");
    edits.push(Edit::insert(candidate.call.as_node().span().start, text));
}

/// The next statement after `node` in its `StatementsNode` parent, if any.
fn right_sibling<'pr>(node: &Node<'pr>, analysis: &Analysis<'pr>) -> Option<Node<'pr>> {
    let parent = analysis.parent_of(node)?;
    let stmts = parent.as_statements_node()?;
    let items: Vec<Node<'pr>> = stmts.body().iter().collect();
    let idx = items.iter().position(|n| same(n, node))?;
    items.get(idx + 1).copied()
}

/// RuboCop's `return_value_used?`.
fn return_value_used(node: Node<'_>, analysis: &Analysis<'_>) -> bool {
    let mut current = node;
    loop {
        let Some(parent) = analysis.parent_of(&current) else { return false };
        if let Some(stmts) = parent.as_statements_node() {
            let items: Vec<Node<'_>> = stmts.body().iter().collect();
            let is_last = items.last().is_some_and(|last| same(last, &current));
            if !is_last {
                return false;
            }
            current = parent;
            continue;
        }
        if matches!(parent.kind(), NodeKind::ParenthesesNode | NodeKind::BeginNode) {
            current = parent;
            continue;
        }
        return match parent.kind() {
            NodeKind::BlockNode => {
                let is_void = analysis
                    .parent_of(&parent)
                    .and_then(|owner| owner.as_call_node())
                    .is_some_and(|c| matches!(c.name().as_slice(), b"each" | b"tap"));
                !is_void
            }
            NodeKind::ForNode | NodeKind::EnsureNode | NodeKind::ProgramNode => false,
            NodeKind::DefNode => {
                let def = parent.as_def_node().expect("kind matched");
                !def_void_context(&def)
            }
            _ => true,
        };
    }
}

/// RuboCop-AST's `DefNode#void_context?`.
fn def_void_context(def: &DefNode<'_>) -> bool {
    let name = def.name().as_slice();
    (def.receiver().is_none() && name == b"initialize") || is_assignment_method_name(name)
}

/// RuboCop-AST's `MethodIdentifierPredicates#assignment_method?`.
fn is_assignment_method_name(name: &[u8]) -> bool {
    name.ends_with(b"=") && !matches!(name, b"==" | b"!=" | b"<=" | b">=" | b"===")
}

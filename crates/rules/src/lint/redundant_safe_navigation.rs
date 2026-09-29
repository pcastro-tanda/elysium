//! `Lint/RedundantSafeNavigation`, ported from RuboCop's
//! `lib/rubocop/cop/lint/redundant_safe_navigation.rb` plus the
//! `Lint::Utils::NilReceiverChecker` helper it uses for `InferNonNilReceiver`.
//!
//! Prism has no parent pointers, so [`Analysis`] does one recursive pass over
//! the whole tree (from `file_start`) building a `(kind, span) -> parent`
//! map, plus the list of every safe-navigation `CallNode` and `OrNode` --
//! (kind, span) is a stable identity per the `ruby_ast` docs, since Prism
//! never hands out two distinct nodes sharing both. Everything else
//! (`cant_be_nil`, `sole_condition_of_parent_if`, ...) is a literal
//! restatement of `NilReceiverChecker` against that map.
//!
//! whitequark's single `:begin` node (eliding itself for one statement) is
//! Prism's `StatementsNode` (always present); whitequark's `:if` node
//! (shared by `if`/`unless`, distinguished by `unless?`) is Prism's separate
//! `IfNode`/`UnlessNode`; and whitequark's `:ensure`/`:rescue` wrap the
//! protected body as a child, while Prism's `BeginNode` holds `statements`/
//! `rescue_clause`/`else_clause`/`ensure_clause` as flat sibling fields.
//! [`sequentially_reached`] restates the *intent* of upstream's
//! `resbody_type?`/`Ensure#branch` checks (never treat a `rescue`/`ensure`
//! clause's own body as evaluated-before its siblings) directly against that
//! flat shape, rather than reproducing the nested-node check literally.
//! `ParenthesesNode` has no upstream counterpart at all (whitequark elides
//! parens); it gets its own transparent case arm, mirroring `StatementsNode`.

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, OrNode};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Redundant safe navigation detected, use `.` instead.";
const MSG_LITERAL: &str = "Redundant safe navigation with default literal detected.";
const MSG_NON_NIL: &str = "Redundant safe navigation on non-nil receiver (detected by analyzing \
previous code/method invocations).";

/// `NilClass`'s own instance methods (`nil.methods` -- `!` is already one of
/// them, so `NilReceiverChecker`'s `+ %i[!]` adds nothing new). Calling one
/// of these through `.` proves nothing about whether the receiver is `nil`,
/// since `nil` itself answers to all of them too.
const NIL_METHODS: &[&[u8]] = &[
    b"!",
    b"!=",
    b"!~",
    b"&",
    b"<=>",
    b"==",
    b"===",
    b"=~",
    b"^",
    b"__id__",
    b"__send__",
    b"class",
    b"clone",
    b"define_singleton_method",
    b"display",
    b"dup",
    b"enum_for",
    b"eql?",
    b"equal?",
    b"extend",
    b"freeze",
    b"frozen?",
    b"hash",
    b"inspect",
    b"instance_eval",
    b"instance_exec",
    b"instance_of?",
    b"instance_variable_defined?",
    b"instance_variable_get",
    b"instance_variable_set",
    b"instance_variables",
    b"is_a?",
    b"itself",
    b"kind_of?",
    b"method",
    b"methods",
    b"nil?",
    b"object_id",
    b"private_methods",
    b"protected_methods",
    b"public_method",
    b"public_methods",
    b"public_send",
    b"rationalize",
    b"remove_instance_variable",
    b"respond_to?",
    b"send",
    b"singleton_class",
    b"singleton_method",
    b"singleton_methods",
    b"tap",
    b"then",
    b"to_a",
    b"to_c",
    b"to_enum",
    b"to_f",
    b"to_h",
    b"to_i",
    b"to_r",
    b"to_s",
    b"yield_self",
    b"|",
];

/// `GUARANTEED_INSTANCE_METHODS`: these always return a real instance of
/// their target type, never `nil`, so chaining `&.` off one is redundant.
const GUARANTEED_INSTANCE_METHODS: &[&[u8]] = &[b"to_s", b"to_i", b"to_f", b"to_a", b"to_h"];

/// Checks for redundant safe navigation calls.
#[derive(Debug, Clone)]
pub struct RedundantSafeNavigation {
    allowed_methods: Vec<String>,
    infer_non_nil_receiver: bool,
    additional_nil_methods: Vec<Vec<u8>>,
}

impl Rule for RedundantSafeNavigation {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RedundantSafeNavigation",
        department: Department::Lint,
        summary: "Checks for redundant safe navigation calls.",
        explanation: "\
Use cases where a constant, named in camel case for classes and modules, is `nil` are rare, and an
offense is not detected when the receiver is a constant. The detection also applies to `self`, and
to literal receivers, except for `nil`.

For all receivers, the `instance_of?`, `kind_of?`, `is_a?`, `eql?`, `respond_to?`, and `equal?`
methods are checked by default. These are customizable with the `AllowedMethods` option.

The `AllowedMethods` option specifies nil-safe methods, i.e. methods that are allowed to skip safe
navigation.

The `InferNonNilReceiver` option specifies whether to look into previous code paths to infer if the
receiver can't be `nil`. This check is unsafe because the receiver can be redefined between the
safe navigation call and the previous regular method call. It does the inference only in the
current scope (e.g. within the same method definition).

The `AdditionalNilMethods` option specifies additional custom methods that are defined on
`NilClass`, used by `InferNonNilReceiver`.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            linter::ConfigOption {
                name: "AllowedMethods",
                default: linter::ConfigDefault::StrList(&[
                    "instance_of?",
                    "kind_of?",
                    "is_a?",
                    "eql?",
                    "respond_to?",
                    "equal?",
                ]),
                allowed: &[],
                doc: "Nil-safe methods allowed to skip safe navigation.",
            },
            linter::ConfigOption {
                name: "InferNonNilReceiver",
                default: linter::ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Infer non-nil receivers from previous code in the same scope.",
            },
            linter::ConfigOption {
                name: "AdditionalNilMethods",
                default: linter::ConfigDefault::StrList(&["present?", "blank?", "try", "try!"]),
                allowed: &[],
                doc: "Custom methods considered defined on `NilClass` for `InferNonNilReceiver`.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allowed_methods: options.str_list("AllowedMethods"),
            infer_non_nil_receiver: options.bool("InferNonNilReceiver"),
            additional_nil_methods: options
                .str_list("AdditionalNilMethods")
                .into_iter()
                .map(String::into_bytes)
                .collect(),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let analysis = Analysis::build(root);
        let mut offenses: Vec<(Span, &'static str, Option<Fix>)> = Vec::new();

        for call in &analysis.csend_calls {
            let Some(dot) = call.call_operator_loc() else { continue };
            let dot_span = dot.span();

            if self.infer_non_nil_receiver {
                if let Some(receiver) = call.receiver() {
                    if cant_be_nil(receiver, &self.additional_nil_methods, &analysis) {
                        offenses.push((dot_span, MSG_NON_NIL, Some(dot_fix(dot_span))));
                        continue;
                    }
                }
            }

            if !guarded_by_nil_receiver(call, &self.allowed_methods, &analysis) {
                offenses.push((dot_span, MSG, Some(dot_fix(dot_span))));
            }
        }

        for or_node in &analysis.or_nodes {
            let Some(call) = conversion_with_default(or_node) else { continue };
            let Some(dot) = call.call_operator_loc() else { continue };
            let dot_span = dot.span();
            let or_end = or_node.as_node().span().end;
            let lhs_end = call.as_node().span().end;
            let offense_span = Span::new(dot_span.start, or_end);
            let fix = Fix {
                applicability: Applicability::Unsafe,
                edits: vec![
                    Edit::replace(dot_span, b".".to_vec()),
                    Edit::delete(Span::new(lhs_end, or_end)),
                ],
            };
            offenses.push((offense_span, MSG_LITERAL, Some(fix)));
        }

        for (span, msg, fix) in offenses {
            match fix {
                Some(fix) => ctx.report_with_fix(&Self::META, span, msg, fix),
                None => ctx.report(&Self::META, span, msg),
            }
        }
    }
}

fn dot_fix(dot_span: Span) -> Fix {
    Fix {
        applicability: Applicability::Unsafe,
        edits: vec![Edit::replace(dot_span, b".".to_vec())],
    }
}

/// `(kind, span) -> parent` map plus every safe-navigation `CallNode`/`OrNode`
/// in the file, built by one recursive descent from the root.
struct Analysis<'pr> {
    parent: HashMap<(NodeKind, Span), Node<'pr>>,
    csend_calls: Vec<CallNode<'pr>>,
    or_nodes: Vec<OrNode<'pr>>,
}

impl<'pr> Analysis<'pr> {
    fn build(root: Node<'pr>) -> Self {
        let mut analysis =
            Self { parent: HashMap::new(), csend_calls: Vec::new(), or_nodes: Vec::new() };
        analysis.visit(root);
        analysis
    }

    fn visit(&mut self, node: Node<'pr>) {
        if let Some(call) = node.as_call_node() {
            if call.is_safe_navigation() {
                self.csend_calls.push(call);
            }
        }
        if let Some(or_node) = node.as_or_node() {
            self.or_nodes.push(or_node);
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

/// Same underlying node: RuboCop's `.equal?` between two `Node` handles
/// (Prism hands out fresh values, but never two distinct nodes sharing both
/// kind and span, per `ruby_ast`'s docs).
fn same_position(a: &Node<'_>, b: &Node<'_>) -> bool {
    a.kind() == b.kind() && a.span() == b.span()
}

/// RuboCop's `Node#==`: structural equality, not identity -- two different
/// occurrences of the same bare variable/call/`it`/`self`.
fn same_shape(a: &Node<'_>, b: &Node<'_>) -> bool {
    if a.kind() != b.kind() {
        return false;
    }
    match a.kind() {
        NodeKind::LocalVariableReadNode => {
            let (a, b) = (
                a.as_local_variable_read_node().expect("kind matched"),
                b.as_local_variable_read_node().expect("kind matched"),
            );
            a.name().as_slice() == b.name().as_slice()
        }
        NodeKind::ItLocalVariableReadNode | NodeKind::SelfNode => true,
        NodeKind::InstanceVariableReadNode => {
            let (a, b) = (
                a.as_instance_variable_read_node().expect("kind matched"),
                b.as_instance_variable_read_node().expect("kind matched"),
            );
            a.name().as_slice() == b.name().as_slice()
        }
        NodeKind::ClassVariableReadNode => {
            let (a, b) = (
                a.as_class_variable_read_node().expect("kind matched"),
                b.as_class_variable_read_node().expect("kind matched"),
            );
            a.name().as_slice() == b.name().as_slice()
        }
        NodeKind::GlobalVariableReadNode => {
            let (a, b) = (
                a.as_global_variable_read_node().expect("kind matched"),
                b.as_global_variable_read_node().expect("kind matched"),
            );
            a.name().as_slice() == b.name().as_slice()
        }
        NodeKind::ConstantReadNode => {
            let (a, b) = (
                a.as_constant_read_node().expect("kind matched"),
                b.as_constant_read_node().expect("kind matched"),
            );
            a.name().as_slice() == b.name().as_slice()
        }
        NodeKind::CallNode => {
            let (ca, cb) =
                (a.as_call_node().expect("kind matched"), b.as_call_node().expect("kind matched"));
            if ca.is_safe_navigation() != cb.is_safe_navigation()
                || ca.name().as_slice() != cb.name().as_slice()
            {
                return false;
            }
            match (ca.receiver(), cb.receiver()) {
                (None, None) => true,
                (Some(ra), Some(rb)) => same_shape(&ra, &rb),
                _ => false,
            }
        }
        _ => false,
    }
}

/// RuboCop's `NilReceiverChecker#binding_name`: the name whose *binding* (not
/// just spelling) can differ between structurally-equal occurrences -- a
/// local variable/block parameter, or bare `it`.
fn binding_name(node: &Node<'_>) -> Option<Vec<u8>> {
    match node.kind() {
        NodeKind::LocalVariableReadNode => Some(
            node.as_local_variable_read_node().expect("kind matched").name().as_slice().to_vec(),
        ),
        NodeKind::ItLocalVariableReadNode => Some(b"it".to_vec()),
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            (call.receiver().is_none()
                && call.arguments().is_none()
                && call.name().as_slice() == b"it")
                .then(|| b"it".to_vec())
        }
        _ => None,
    }
}

/// RuboCop's `NilReceiverChecker#block_binds_name?`.
fn block_binds_name(name: &[u8], block: &ruby_ast::node::BlockNode<'_>) -> bool {
    match block.parameters() {
        None => name == b"it",
        Some(params) => match params.kind() {
            NodeKind::ItParametersNode => name == b"it",
            NodeKind::NumberedParametersNode => {
                let np = params.as_numbered_parameters_node().expect("kind matched");
                name.len() == 2
                    && name[0] == b'_'
                    && name[1].is_ascii_digit()
                    && (1..=np.maximum()).contains(&(name[1] - b'0'))
            }
            NodeKind::BlockParametersNode => {
                let bp = params.as_block_parameters_node().expect("kind matched");
                match bp.parameters() {
                    Some(p) => parameters_include(&p, name),
                    None => name == b"it",
                }
            }
            _ => false,
        },
    }
}

fn parameters_include(params: &ruby_ast::node::ParametersNode<'_>, name: &[u8]) -> bool {
    params.requireds().iter().any(|n| param_name_eq(&n, name))
        || params.optionals().iter().any(|n| param_name_eq(&n, name))
        || params.posts().iter().any(|n| param_name_eq(&n, name))
        || params.rest().is_some_and(|n| param_name_eq(&n, name))
        || params.keywords().iter().any(|n| param_name_eq(&n, name))
        || params.keyword_rest().is_some_and(|n| param_name_eq(&n, name))
        || params.block().is_some_and(|n| param_name_eq(&n.as_node(), name))
}

fn param_name_eq(node: &Node<'_>, name: &[u8]) -> bool {
    match node.kind() {
        NodeKind::RequiredParameterNode => {
            node.as_required_parameter_node().expect("kind matched").name().as_slice() == name
        }
        NodeKind::OptionalParameterNode => {
            node.as_optional_parameter_node().expect("kind matched").name().as_slice() == name
        }
        NodeKind::RequiredKeywordParameterNode => {
            node.as_required_keyword_parameter_node().expect("kind matched").name().as_slice()
                == name
        }
        NodeKind::OptionalKeywordParameterNode => {
            node.as_optional_keyword_parameter_node().expect("kind matched").name().as_slice()
                == name
        }
        NodeKind::RestParameterNode => node
            .as_rest_parameter_node()
            .expect("kind matched")
            .name()
            .is_some_and(|n| n.as_slice() == name),
        NodeKind::KeywordRestParameterNode => node
            .as_keyword_rest_parameter_node()
            .expect("kind matched")
            .name()
            .is_some_and(|n| n.as_slice() == name),
        NodeKind::BlockParameterNode => node
            .as_block_parameter_node()
            .expect("kind matched")
            .name()
            .is_some_and(|n| n.as_slice() == name),
        _ => false,
    }
}

/// RuboCop's `NilReceiverChecker#binding_scope`: the nearest enclosing block
/// (that binds `name`) or method/class/module/singleton-class boundary,
/// identified by its `(kind, span)`; `None` means the top-level scope.
fn binding_scope_of<'pr>(
    mut node: Node<'pr>,
    name: &[u8],
    analysis: &Analysis<'pr>,
) -> Option<(NodeKind, Span)> {
    loop {
        let parent = analysis.parent_of(&node)?;
        match parent.kind() {
            NodeKind::BlockNode => {
                let block = parent.as_block_node().expect("kind matched");
                let is_body = block.body().is_some_and(|b| same_position(&b, &node));
                if is_body && block_binds_name(name, &block) {
                    return Some((parent.kind(), parent.span()));
                }
            }
            NodeKind::DefNode
            | NodeKind::ClassNode
            | NodeKind::ModuleNode
            | NodeKind::SingletonClassNode => {
                return Some((parent.kind(), parent.span()));
            }
            _ => {}
        }
        node = parent;
    }
}

/// The receiver's binding identity, computed once per offending call.
struct ReceiverBinding {
    name: Option<Vec<u8>>,
    scope: Option<(NodeKind, Span)>,
}

impl ReceiverBinding {
    fn compute(receiver: &Node<'_>, analysis: &Analysis<'_>) -> Self {
        match binding_name(receiver) {
            Some(name) => {
                let scope = binding_scope_of(*receiver, &name, analysis);
                Self { name: Some(name), scope }
            }
            None => Self { name: None, scope: None },
        }
    }

    fn same_binding_as_receiver(&self, occurrence: &Node<'_>, analysis: &Analysis<'_>) -> bool {
        match &self.name {
            None => true,
            Some(name) => binding_scope_of(*occurrence, name, analysis) == self.scope,
        }
    }
}

/// RuboCop's `NilReceiverChecker#csend_root_receiver`.
fn csend_root_receiver<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let mut receiver = call.receiver()?;
    while let Some(inner_call) = receiver.as_call_node() {
        let Some(inner_receiver) = inner_call.receiver() else { break };
        receiver = inner_receiver;
    }
    Some(receiver)
}

/// RuboCop's `NilReceiverChecker#non_nil_condition?`.
fn non_nil_condition(
    condition: &Node<'_>,
    node: &Node<'_>,
    binding: &ReceiverBinding,
    analysis: &Analysis<'_>,
) -> bool {
    if same_shape(condition, node) && binding.same_binding_as_receiver(condition, analysis) {
        return true;
    }
    let Some(call) = condition.as_call_node() else { return false };
    if !call.is_safe_navigation() {
        return false;
    }
    let Some(root) = csend_root_receiver(&call) else { return false };
    same_shape(&root, node) && binding.same_binding_as_receiver(&root, analysis)
}

/// RuboCop's generic `else_branch?`: is `node` exactly its parent `IfNode`'s
/// `subsequent` (which covers both a nested `elsif` and a terminal `else`)
/// or its parent `UnlessNode`'s `else_clause`?
fn is_else_branch(node: &Node<'_>, analysis: &Analysis<'_>) -> bool {
    let Some(parent) = analysis.parent_of(node) else { return false };
    match parent.kind() {
        NodeKind::IfNode => parent
            .as_if_node()
            .expect("kind matched")
            .subsequent()
            .is_some_and(|s| same_position(&s, node)),
        NodeKind::UnlessNode => parent
            .as_unless_node()
            .expect("kind matched")
            .else_clause()
            .is_some_and(|e| same_position(&e.as_node(), node)),
        _ => false,
    }
}

/// RuboCop's `elsif?`: an `IfNode` reached as its parent `IfNode`'s
/// `subsequent` (a real terminal `else` is an `ElseNode`, never an `IfNode`,
/// so this never matches one; a ternary's `IfNode` is never any node's
/// `subsequent`, so it never matches either).
fn is_elsif(node: &Node<'_>, analysis: &Analysis<'_>) -> bool {
    node.as_if_node().is_some() && is_else_branch(node, analysis)
}

/// RuboCop's `find_top_if`.
fn find_top_if<'pr>(mut node: Node<'pr>, analysis: &Analysis<'pr>) -> Node<'pr> {
    while is_elsif(&node, analysis) {
        node = analysis.parent_of(&node).expect("elsif has a parent");
    }
    node
}

/// RuboCop's `NilReceiverChecker#sole_condition_of_parent_if?`.
fn sole_condition_of_parent_if(
    receiver: Node<'_>,
    binding: &ReceiverBinding,
    analysis: &Analysis<'_>,
) -> bool {
    let mut child = receiver;
    let mut parent = analysis.parent_of(&receiver);
    while let Some(p) = parent {
        let next_parent = if let Some(if_node) = p.as_if_node() {
            let condition = if_node.predicate();
            if !same_position(&child, &condition)
                && non_nil_condition(&condition, &receiver, binding, analysis)
            {
                return true;
            }
            if is_elsif(&p, analysis) {
                Some(find_top_if(p, analysis))
            } else {
                Some(p)
            }
        } else if is_else_branch(&p, analysis) {
            analysis.parent_of(&p)
        } else {
            Some(p)
        };
        match next_parent {
            Some(np) => {
                child = np;
                parent = analysis.parent_of(&np);
            }
            None => parent = None,
        }
    }
    false
}

/// RuboCop's `NilReceiverChecker#non_nil_method?`.
fn non_nil_method(name: &[u8], additional_nil_methods: &[Vec<u8>]) -> bool {
    !NIL_METHODS.contains(&name) && !additional_nil_methods.iter().any(|m| m.as_slice() == name)
}

/// Restates the *intent* of upstream's `resbody_type?`/`Ensure#branch`
/// checks against Prism's flat `BeginNode` shape: a `rescue`/`ensure`
/// clause's own body is never "evaluated before" its siblings (an exception
/// may have interrupted the protected body first), matching whitequark's
/// `resbody`/ensure-branch exclusions without their nested-node shape.
fn sequentially_reached(node: &Node<'_>, analysis: &Analysis<'_>) -> bool {
    if matches!(node.kind(), NodeKind::RescueNode | NodeKind::EnsureNode) {
        return false;
    }
    if let Some(parent) = analysis.parent_of(node) {
        if let Some(modifier) = parent.as_rescue_modifier_node() {
            if same_position(&modifier.rescue_expression(), node) {
                return false;
            }
        }
    }
    !is_else_branch(node, analysis)
        || (node.kind() == NodeKind::IfNode && !is_elsif(node, analysis))
}

fn left_siblings<'pr>(node: &Node<'pr>, parent: &Node<'pr>) -> Vec<Node<'pr>> {
    let mut all = Vec::new();
    for_each_child(parent, |c| all.push(*c));
    match all.iter().position(|c| same_position(c, node)) {
        Some(pos) => {
            all.truncate(pos);
            all
        }
        None => Vec::new(),
    }
}

/// The single child RuboCop's generic recursive arms delegate to (`if`/`unless`/`while`/`until`/
/// `case`/`case_match`'s predicate, `and`/`or`'s lhs, a `begin`'s first statement, parens' body, an
/// assignment's value).
fn single_child_arm<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    match node.kind() {
        NodeKind::IfNode => Some(node.as_if_node().expect("kind matched").predicate()),
        NodeKind::UnlessNode => Some(node.as_unless_node().expect("kind matched").predicate()),
        NodeKind::WhileNode => Some(node.as_while_node().expect("kind matched").predicate()),
        NodeKind::UntilNode => Some(node.as_until_node().expect("kind matched").predicate()),
        NodeKind::CaseNode => node.as_case_node().expect("kind matched").predicate(),
        NodeKind::CaseMatchNode => node.as_case_match_node().expect("kind matched").predicate(),
        NodeKind::AndNode => Some(node.as_and_node().expect("kind matched").left()),
        NodeKind::OrNode => Some(node.as_or_node().expect("kind matched").left()),
        NodeKind::StatementsNode => node.as_statements_node().expect("kind matched").body().first(),
        NodeKind::ParenthesesNode => node.as_parentheses_node().expect("kind matched").body(),
        NodeKind::LocalVariableWriteNode => {
            Some(node.as_local_variable_write_node().expect("kind matched").value())
        }
        NodeKind::InstanceVariableWriteNode => {
            Some(node.as_instance_variable_write_node().expect("kind matched").value())
        }
        NodeKind::ClassVariableWriteNode => {
            Some(node.as_class_variable_write_node().expect("kind matched").value())
        }
        NodeKind::GlobalVariableWriteNode => {
            Some(node.as_global_variable_write_node().expect("kind matched").value())
        }
        NodeKind::ConstantWriteNode => {
            Some(node.as_constant_write_node().expect("kind matched").value())
        }
        NodeKind::ConstantPathWriteNode => {
            Some(node.as_constant_path_write_node().expect("kind matched").value())
        }
        _ => None,
    }
}

/// The multiple children RuboCop's remaining generic recursive arms check in order (a `pair`'s
/// key and value, a `when`'s conditions).
fn multi_child_arm<'pr>(node: &Node<'pr>) -> Vec<Node<'pr>> {
    match node.kind() {
        NodeKind::AssocNode => {
            let assoc = node.as_assoc_node().expect("kind matched");
            vec![assoc.key(), assoc.value()]
        }
        NodeKind::WhenNode => {
            node.as_when_node().expect("kind matched").conditions().iter().collect()
        }
        _ => Vec::new(),
    }
}

/// RuboCop's `:send` case arm: the one arm with a hard early return (`Some`) distinct from
/// falling through to the generic siblings/parent walk (`None`).
fn call_arm_evidence(
    call: &CallNode<'_>,
    receiver: Node<'_>,
    additional_nil_methods: &[Vec<u8>],
    binding: &ReceiverBinding,
    analysis: &Analysis<'_>,
    checked: &mut HashSet<(NodeKind, Span)>,
) -> Option<bool> {
    if !call.is_safe_navigation() {
        if let Some(recv) = call.receiver() {
            if same_shape(&recv, &receiver) && binding.same_binding_as_receiver(&recv, analysis) {
                return Some(non_nil_method(call.name().as_slice(), additional_nil_methods));
            }
        }
    }
    if let Some(args) = call.arguments() {
        for arg in &args.arguments() {
            if cant_be_nil_rec(arg, receiver, additional_nil_methods, binding, analysis, checked) {
                return Some(true);
            }
        }
    }
    if let Some(recv) = call.receiver() {
        if cant_be_nil_rec(recv, receiver, additional_nil_methods, binding, analysis, checked) {
            return Some(true);
        }
    }
    None
}

/// RuboCop's `NilReceiverChecker#_cant_be_nil?`.
fn cant_be_nil_rec(
    node: Node<'_>,
    receiver: Node<'_>,
    additional_nil_methods: &[Vec<u8>],
    binding: &ReceiverBinding,
    analysis: &Analysis<'_>,
    checked: &mut HashSet<(NodeKind, Span)>,
) -> bool {
    if !checked.insert((node.kind(), node.span())) {
        return false;
    }
    if matches!(
        node.kind(),
        NodeKind::DefNode
            | NodeKind::ClassNode
            | NodeKind::ModuleNode
            | NodeKind::SingletonClassNode
    ) {
        return false;
    }

    if let Some(call) = node.as_call_node() {
        if let Some(result) =
            call_arm_evidence(&call, receiver, additional_nil_methods, binding, analysis, checked)
        {
            return result;
        }
    } else {
        for child in single_child_arm(&node).into_iter().chain(multi_child_arm(&node)) {
            if cant_be_nil_rec(child, receiver, additional_nil_methods, binding, analysis, checked)
            {
                return true;
            }
        }
    }

    if sequentially_reached(&node, analysis) {
        if let Some(parent) = analysis.parent_of(&node) {
            for sibling in left_siblings(&node, &parent).into_iter().rev() {
                if cant_be_nil_rec(
                    sibling,
                    receiver,
                    additional_nil_methods,
                    binding,
                    analysis,
                    checked,
                ) {
                    return true;
                }
            }
        }
    }

    match analysis.parent_of(&node) {
        Some(parent) => {
            cant_be_nil_rec(parent, receiver, additional_nil_methods, binding, analysis, checked)
        }
        None => false,
    }
}

/// RuboCop's `NilReceiverChecker#cant_be_nil?`.
fn cant_be_nil(
    receiver: Node<'_>,
    additional_nil_methods: &[Vec<u8>],
    analysis: &Analysis<'_>,
) -> bool {
    let binding = ReceiverBinding::compute(&receiver, analysis);
    if sole_condition_of_parent_if(receiver, &binding, analysis) {
        return true;
    }
    let Some(parent) = analysis.parent_of(&receiver) else { return false };
    let mut checked = HashSet::new();
    cant_be_nil_rec(parent, receiver, additional_nil_methods, &binding, analysis, &mut checked)
}

/// RuboCop's `SNAKE_CASE` regex (`\A[[:digit:][:upper:]_]+\z`).
fn is_snake_case(name: &[u8]) -> bool {
    !name.is_empty()
        && name.iter().all(|&b| b.is_ascii_digit() || b.is_ascii_uppercase() || b == b'_')
}

fn is_literal_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::RangeNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}

/// RuboCop's `#assume_receiver_instance_exists?`.
fn assume_receiver_instance_exists(receiver: &Node<'_>) -> bool {
    match receiver.kind() {
        NodeKind::ConstantReadNode => !is_snake_case(
            receiver.as_constant_read_node().expect("kind matched").name().as_slice(),
        ),
        NodeKind::ConstantPathNode => receiver
            .as_constant_path_node()
            .expect("kind matched")
            .name()
            .is_some_and(|n| !is_snake_case(n.as_slice())),
        NodeKind::SelfNode => true,
        _ => is_literal_kind(receiver.kind()) && receiver.kind() != NodeKind::NilNode,
    }
}

/// RuboCop's `#guaranteed_instance?`.
fn guaranteed_instance(receiver: &Node<'_>) -> bool {
    receiver.as_call_node().is_some_and(|call| {
        !call.is_safe_navigation() && GUARANTEED_INSTANCE_METHODS.contains(&call.name().as_slice())
    })
}

fn is_condition_position(parent: &Node<'_>, node: &Node<'_>) -> bool {
    let predicate = match parent.kind() {
        NodeKind::IfNode => Some(parent.as_if_node().expect("kind matched").predicate()),
        NodeKind::UnlessNode => Some(parent.as_unless_node().expect("kind matched").predicate()),
        NodeKind::WhileNode => Some(parent.as_while_node().expect("kind matched").predicate()),
        NodeKind::UntilNode => Some(parent.as_until_node().expect("kind matched").predicate()),
        NodeKind::CaseNode => parent.as_case_node().expect("kind matched").predicate(),
        NodeKind::CaseMatchNode => parent.as_case_match_node().expect("kind matched").predicate(),
        _ => None,
    };
    predicate.is_some_and(|p| same_position(&p, node))
}

fn is_negation_parent(parent: &Node<'_>, node: &Node<'_>) -> bool {
    parent.as_call_node().is_some_and(|call| {
        !call.is_safe_navigation()
            && call.name().as_slice() == b"!"
            && call.receiver().is_some_and(|r| same_position(&r, node))
    })
}

/// RuboCop's `#check?`/`#condition?`.
fn check(call: &CallNode<'_>, allowed_methods: &[String], analysis: &Analysis<'_>) -> bool {
    if !allowed_methods.iter().any(|m| m.as_bytes() == call.name().as_slice()) {
        return false;
    }
    let node = call.as_node();
    let Some(parent) = analysis.parent_of(&node) else { return false };
    is_condition_position(&parent, &node)
        || matches!(parent.kind(), NodeKind::AndNode | NodeKind::OrNode)
        || is_negation_parent(&parent, &node)
}

/// RuboCop's `#respond_to_nil_method?`.
fn respond_to_nil_method(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"respond_to?" {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let list = args.arguments();
    if list.len() != 1 {
        return false;
    }
    list.first()
        .and_then(|a| a.as_symbol_node())
        .is_some_and(|sym| NIL_METHODS.contains(&sym.unescaped()))
}

/// RuboCop's `#guarded_by_nil_receiver?`.
fn guarded_by_nil_receiver(
    call: &CallNode<'_>,
    allowed_methods: &[String],
    analysis: &Analysis<'_>,
) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    if assume_receiver_instance_exists(&receiver) {
        return false;
    }
    let is_guaranteed = guaranteed_instance(&receiver);
    if !is_guaranteed && !check(call, allowed_methods, analysis) {
        return true;
    }
    respond_to_nil_method(call) && !is_guaranteed
}

fn is_empty_hash(n: &Node<'_>) -> bool {
    n.as_hash_node().is_some_and(|h| h.elements().is_empty())
}

fn is_empty_array(n: &Node<'_>) -> bool {
    n.as_array_node().is_some_and(|a| a.elements().is_empty())
}

fn is_int_zero(n: &Node<'_>) -> bool {
    n.as_integer_node().is_some_and(|i| TryInto::<i32>::try_into(i.value()) == Ok(0))
}

fn is_float_zero(n: &Node<'_>) -> bool {
    n.as_float_node().is_some_and(|f| f.value() == 0.0)
}

fn is_empty_string(n: &Node<'_>) -> bool {
    n.as_string_node().is_some_and(|s| s.unescaped().is_empty())
}

/// RuboCop's `#conversion_with_default?`.
fn conversion_with_default<'pr>(or_node: &OrNode<'pr>) -> Option<CallNode<'pr>> {
    let lhs = or_node.left();
    let call = lhs.as_call_node()?;
    if !call.is_safe_navigation() {
        return None;
    }
    let rhs = or_node.right();
    let matches = match call.name().as_slice() {
        b"to_h" => is_empty_hash(&rhs),
        b"to_a" => is_empty_array(&rhs),
        b"to_i" => is_int_zero(&rhs),
        b"to_f" => is_float_zero(&rhs),
        b"to_s" => is_empty_string(&rhs),
        _ => false,
    };
    matches.then_some(call)
}

# Phase 4: semantic layer and the ten semantic cops

Status: design complete, implementation starting (2026-09-25).

## Scope

1. `crates/ruby_semantic`: a port of RuboCop's `VariableForce`
   (`lib/rubocop/cop/variable_force.rb` + `variable_force/*.rb`, 1.82.1)
   onto Prism nodes. Lexical scopes, local-variable declarations,
   assignments, references, block capture, and the `Branch` model used to
   decide whether an assignment is dead.
2. `linter::Context::semantics()`: lazy `OnceCell<Semantics<'a>>` built on
   first use by a second traversal of `Parsed::root()`. Files where no
   semantic cop is enabled pay nothing; the walk is ~5% of parse cost (see
   STATUS.md micro benchmarks), so a second walk stays inside budget. ADR
   0007 records this as the one sanctioned exception to ADR 0001's single
   traversal.
3. The ten cops excluded from Phase 3 (`phase3-rules.md`):

| cop | needs `ruby_semantic`? | notes |
|---|---|---|
| Lint/UselessAssignment | yes (full model) | 145 spec examples; autocorrect |
| Lint/ShadowingOuterLocalVariable | yes (`Variable::shadows`) | |
| Style/RedundantSelf | no | RuboCop does not use VariableForce; port its `@local_variables_scopes` heuristic verbatim (see below) |
| Lint/ShadowedException | no | needs a built-in exception hierarchy table |
| Lint/UselessAccessModifier | no | class-body scan |
| Lint/MissingSuper | no | syntactic |
| Lint/ConstantResolution | no | syntactic, disabled by default |
| Lint/NumberConversion | no | syntactic + AllowedMethods/AllowedPattern |
| Lint/SelfAssignment | no | syntactic |
| Style/OptionalBooleanParameter | no | syntactic + AllowedMethods |

Fixtures for all ten are generated with `tools/port_spec.rb` (ADR 0006).

## `ruby_semantic` design

Arena-style, id-indexed, borrowing the tree (`Semantics<'pr>`). Nodes are
stored directly (`Node<'pr>`; vendored Prism gains `Clone + Copy` derives on
`Node` and every node struct — additive, ADR 0002 addendum). Identity is
`(NodeKind, Span)`; rules match semantic records against visited nodes via
`node.span()`.

```rust
pub struct Semantics<'pr> { scopes: Vec<Scope<'pr>>, variables: Vec<Variable<'pr>>, assignments: Vec<Assignment<'pr>>, branches: Vec<Branch<'pr>> }
pub struct ScopeId(u32); VariableId(u32); AssignmentId(u32); BranchId(u32);

pub struct Scope<'pr> {
    node: Node<'pr>,            // ProgramNode (naked top level) | DefNode | BlockNode | LambdaNode | ClassNode | ModuleNode | SingletonClassNode
    parent: Option<ScopeId>,
    kind: ScopeKind,            // TopLevel | Def | Block (BlockNode+LambdaNode) | Class | Module | SingletonClass
    variables: Vec<VariableId>, // declaration order
    bare_call_names: Vec<&'pr [u8]>, // receiverless, argumentless CallNodes directly in this scope (UselessAssignment's `collect_variable_like_names`)
    ancestors: Box<[Node<'pr>]>,     // outermost-first, to root
}
impl Scope { fn body(&self) -> Option<Node>; fn return_value_node(&self) -> Option<Node> /* last statement of body StatementsNode, else body */; fn is_block(&self) -> bool }

pub struct Variable<'pr> {
    name: &'pr [u8],
    declaration: Node<'pr>,     // parameter node | LocalVariableWriteNode | LocalVariableTargetNode | Local*WriteNode | MatchWriteNode
    decl_kind: DeclKind,        // RequiredArg | OptionalArg | RestArg | KeywordArg | OptionalKeywordArg | KeywordRestArg | BlockArg | BlockLocal | Assignment | RegexpNamedCapture | PatternMatch
    scope: ScopeId,
    assignments: Vec<AssignmentId>,
    references: Vec<Reference<'pr>>,
    captured_by_block: bool,
    shadows: Option<VariableId>, // `find_variable(name)` result at declaration time (ShadowingOuterLocalVariable's before_declaring_variable)
}
impl Variable { referenced, used (captured||referenced), should_be_unused (name starts with `_`), is_argument, is_method_argument (argument && scope is Def), is_block_argument, is_keyword_argument, is_explicit_block_local }

pub struct Assignment<'pr> {
    node: Node<'pr>,            // LocalVariableWriteNode | LocalVariableTargetNode | Local{And,Or,Operator}WriteNode | MatchWriteNode
    variable: VariableId,
    name_span: Span,            // name_loc; for MatchWriteNode the regexp (call.receiver) span
    referenced: bool, references: Vec<Node<'pr>>, reassigned: bool,
    branch: Option<BranchId>,
    meta: Option<Meta>,         // Operator{op: &'pr [u8], op_span} | Multiple(MultiWriteNode) | Rest(SplatNode) | For(ForNode)
    exception: bool,            // RescueNode.reference
    ancestors: Box<[Node<'pr>]>,// outermost-first, to root (UselessAssignment walks parents)
}
impl Assignment { used = (!reassigned && var.captured_by_block) || referenced; operator() }

pub struct Reference<'pr> { node: Node<'pr>, branch: Option<BranchId>, explicit: bool /* false for ForwardingSuperNode and `binding` */ }

pub struct Branch<'pr> { control: Node<'pr>, child: Node<'pr>, kind: BranchKind, parent: Option<BranchId>, always_run: bool, may_jump_to_other_branch: bool, may_run_incompletely: bool }
// exclusive_with(other): walk other's chain incl. self; if control == control → child != child; else recurse on parent; false if may_jump_to_other_branch.
```

Builder walks the tree with its own ancestor stack of `Node<'pr>`
(recursive, explicit child ordering) and mirrors `VariableForce` handler by
handler:

| parser type | Prism | handling |
|---|---|---|
| `lvasgn` (with value) | `LocalVariableWriteNode` | declare if absent → walk value → assign |
| `lvasgn` (target) | `LocalVariableTargetNode` under `MultiWriteNode`/`MultiTargetNode`/`SplatNode`/`ForNode.index`/`RescueNode.reference` | declare if absent → assign |
| `match_var` | `LocalVariableTargetNode` inside `InNode`/`MatchPredicateNode`/`MatchRequiredNode` patterns (incl. `CapturePatternNode.target`) | declare only, never assign (`process_pattern_match_variable`) |
| `match_with_lvasgn` | `MatchWriteNode { call, targets }` | declare absent targets → walk `call.arguments` → walk `call.receiver` → assign each; `Assignment.node` is the `MatchWriteNode`, `decl_kind = RegexpNamedCapture` |
| `or_asgn`/`and_asgn`/`op_asgn` on lvasgn | `LocalVariable{Or,And,Operator}WriteNode` | declare if absent → reference → walk value → assign; `meta = Operator` |
| `masgn` | `MultiWriteNode` | walk `value` first, then targets |
| `lvar` | `LocalVariableReadNode` | reference (ignore `ItLocalVariableReadNode`) |
| args | `Required/Optional/Rest/RequiredKeyword/OptionalKeyword/KeywordRest/BlockParameterNode`, `BlockLocalVariableNode`; `MultiTargetNode` in params recurses | declare (skip nameless rest/kwrest/block) |
| `while`/`until` | `WhileNode`/`UntilNode` (`begin_modifier` flag ⇒ post-condition: body before predicate) | `process_loop` |
| `for` | `ForNode`: collection → index → statements | `process_loop` |
| `rescue` with `retry` in a clause | `BeginNode` whose rescue chain contains `RetryNode` | treated as loop |
| `zsuper` | `ForwardingSuperNode` | reference every accessible method argument |
| `send :binding` (no args, not `&.`) | `CallNode` | reference every accessible variable |
| scopes | `DefNode` (receiver twisted), `BlockNode`, `LambdaNode`, `ClassNode` (constant_path + superclass twisted), `ModuleNode` (constant_path twisted), `SingletonClassNode` (expression twisted) | push scope; twisted children walked in the outer scope first |

`find_variable`/`accessible_variables` walk the scope stack outward and stop
after the first non-block scope. Block capture: assignment/reference from a
block scope to a variable of another scope sets `captured_by_block`.

Loop post-processing (`mark_assignments_as_referenced_in_loop`): scan all
descendants (crossing scopes) for reads/writes; for each read name with
assignments inside the loop, mark all assignments under an `if`/`case`/
`case_match`/rescue-`BeginNode` ancestor, plus the last one, as referenced by
the loop node.

Branch computation (`Branch.of`): from the node walk up the builder's
ancestor stack without crossing the enclosing scope node; the first (parent,
child) pair whose parent is a control node and whose child is not the
always-run child yields the branch; `parent` continues the walk from the
control node. Control nodes: `IfNode`/`UnlessNode` (always-run: predicate),
`WhileNode`/`UntilNode` (predicate), `CaseNode`/`CaseMatchNode` (predicate),
`ForNode` (index, collection), `AndNode`/`OrNode` (left), `BeginNode` with
`rescue_clause` (Rescue role: children = statements | each `RescueNode` in the
`subsequent` chain, flattened | `else_clause`; never always-run; main body
may jump / may run incompletely), `BeginNode` with `ensure_clause` (Ensure
role: main = anything but the `EnsureNode`; always-run: ensure body). A
`BeginNode` with both is two nested branches, Rescue inside Ensure, exactly
like parser's `(ensure (rescue ...))`.

`Variable::in_modifier_conditional?`: parent skipping one `StatementsNode`
(and a `ParenthesesNode` + its `StatementsNode`) is a modifier
`IfNode`/`UnlessNode`/`WhileNode`/`UntilNode` (statements precede predicate,
no `begin_modifier`).

Known deviation: for `some_method(foo = 1) do ... end` inside an `if`, parser
stops `Branch.of` at the twisted call node (branch = nil); we continue to the
`if`. Rare; conformance will tell.

## Style/RedundantSelf model

RuboCop keeps `@local_variables_scopes: Hash[node → names]`, populated on
`on_def`/`on_defs` (every descendant shares one fresh list), `on_block`
(descendants share `scopes[block]`, i.e. the enclosing def's list or a fresh
one at top level), `on_args`/`on_blockarg` (push names to the arg node's
list), `on_lvasgn`/`on_masgn`/`on_or_asgn`/`on_and_asgn` (push the LHS name
to `scopes[rhs]`, or to each argument's list when rhs is a call with
arguments), `on_in_pattern`, and `on_if`/`on_while`/`on_until` (pre-populate
from descendant assignments into `scopes[condition]`). `self.foo` is allowed
when `foo` is in the list of the node or any ancestor, or is a `Kernel`
method, or the send is an op-assign LHS. Traversal-order dependent (an
`lvasgn` after the `self.foo` does not exempt it). Port as an ancestor-stack
of list ids keyed by `(kind, span)`; no `ruby_semantic` dependency.

## Verification

- Fixtures: `crates/rules/tests/fixtures.rs` (offenses, corrections,
  idempotence) per ADR 0006.
- `ruby_semantic` unit tests port the assertions of
  `spec/rubocop/cop/variable_force_spec.rb` and `variable_force/*_spec.rb`.
- Conformance: `cargo xtask conformance --app <discourse|mastodon> --rule
  Cop` for every cop before promotion past `nursery`.

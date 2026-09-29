//! `Style/ParallelAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/style/parallel_assignment.rb` plus its private
//! `AssignmentSorter`/`GenericCorrector`/`RescueCorrector`/`ModifierCorrector`
//! helper classes.
//!
//! # `Array(rhs).compact`
//!
//! Upstream's `on_masgn` computes `rhs_elements = Array(rhs).compact`, where
//! `Array()` on any `Parser::AST::Node` calls its `to_a` alias for
//! `#children` (`ast` gem) -- *not* "the elements if this is an array". For a
//! non-array RHS (`a, b = foo`), that yields the *send node's own children*
//! (`[nil, :foo]`, compacted to `[:foo]`), an object never actually compared
//! structurally to anything; the only place `rhs_elements` matters
//! (`allowed_masign?`'s size check) is unreachable whenever `rhs` is not
//! array-shaped, because `allowed_rhs?(rhs)` (`!rhs.array_type? || ...`)
//! already short-circuits the `||` chain first. So this port never
//! reconstructs that quirk: a non-`ArrayNode` RHS is simply always allowed,
//! and `rhs_elements` here means "the RHS array's own elements".
//!
//! # LHS target flattening
//!
//! `MlhsNode#assignments` (`node.assignments` on the `masgn`) flat-maps its
//! immediate children: a *named* splat (`*b`) unwraps to its inner target,
//! an *anonymous* splat (`*`) stays a splat node (so `allowed_lhs?`'s
//! `any?(&:splat_type?)` only ever fires for the anonymous form), and a
//! nested `mlhs` (parenthesized destructuring) recurses. [`flatten_targets`]
//! mirrors this over Prism's `lefts`/`rest`/`rights` triad (`MultiWriteNode`
//! and `MultiTargetNode` share the same three fields).
//!
//! # Dependency tracking is asymmetric by design (matching a real upstream gap)
//!
//! `AssignmentSorter#dependency?` only investigates a LHS target's node
//! *type*: `var_name`'s pattern (`(casgn _ $_) | (_ $_)`) matches whitequark's
//! `casgn`/`lvasgn`/`ivasgn`/`cvasgn`/`gvasgn` (single- or name-plus-scope
//! children), and `accesses?` is only ever consulted when
//! `lhs.send_type?` -- true for an attribute-writer target (`obj.attr=`) but
//! **never** for an index-writer target (`ary[0]=`), which whitequark parses
//! as its own `indexasgn` type, distinct from `send`. So index/index target
//! pairs are never reordered relative to each other, only relative to
//! attribute writes. Prism represents *both* shapes as an ordinary
//! `CallNode` (`[]=`/`attr=`), so [`dependency`] replicates the exemption by
//! name: any target `CallNode` whose method is literally `[]=` is excluded
//! from both checks, exactly like an `indexasgn` upstream.
//!
//! # `add_self_to_getters`
//!
//! Before sorting, upstream rewrites every bare, argument-less, receiver-less
//! RHS call (`implicit_self_getter?`, `(send nil? $_)`) into a synthetic
//! `(send (self) name)` node, so that `self.a, self.b = b, a`'s `accesses?`
//! check (which compares `lhs.receiver` structurally) can match a bare `b`
//! against `self.b=`'s `self` receiver. The substituted nodes are used only
//! for *this* comparison -- the order `AssignmentSorter#tsort` returns still
//! references the original RHS nodes for building the actual correction text
//! (per fixtures, every case that hits this substitution is cyclic and thus
//! never reaches the correction step anyway). [`call_matches_getter`] folds
//! the substitution directly into the receiver comparison (a bare call
//! matches when the LHS receiver is literally `self`) instead of building a
//! synthetic node, which would have no real `Span` to render from.
//!
//! # Corrector selection
//!
//! Upstream picks one of three private corrector classes per masgn, using
//! whitequark-specific structure (`node.parent&.rescue_type?`,
//! `rescue_modifier?` consulting the *token* stream to distinguish a real
//! modifier `rescue` from a `begin/rescue/end` block's `rescue` keyword,
//! which read the same as AST nodes once whitequark elides a single-statement
//! `begin`). Prism sidesteps all of that by giving the two rescue shapes
//! different node kinds outright: `x rescue y` is a [`NodeKind::RescueModifierNode`],
//! while a real or implicit-`def` `begin/rescue/end` block is a
//! [`NodeKind::BeginNode`] with a `rescue_clause`. Consequently:
//!
//! - `masgn.value()` is a `RescueModifierNode`: this can only be genuine
//!   modifier-rescue syntax. [`rescue_correction`] builds the
//!   `RescueCorrector` text, additionally checking whether the masgn is the
//!   sole statement of a `def`'s body (no `BeginNode` in between -- the
//!   `def`'s implicit rescue is spelled out directly) to choose between
//!   appending a bare `rescue`/handler (no `begin`/`end` needed) and wrapping
//!   in an explicit `begin ... end`.
//! - `masgn.value()` is a plain `ArrayNode`, and the masgn happens to be the
//!   sole statement of a `begin/rescue/end` (explicit or implicit-`def`)
//!   block: no special handling is needed at all -- the default
//!   `GenericCorrector` path (replace just the masgn's own span) already
//!   does the right thing, since the surrounding `begin`/`rescue`/`end` text
//!   is untouched either way. This is why this file has no code path for it.
//! - Otherwise, if the masgn's sole enclosing statement is a modifier-form
//!   `if`/`unless`/`while`/`until` (detected the same way
//!   `redundant_require_statement.rs` detects one, via ancestor span
//!   comparison -- see [`modifier_ancestor`]): [`modifier_correction`] wraps
//!   in `keyword cond\n  ...\nend`.
//! - Otherwise: plain `GenericCorrector` (replace the masgn's own span with
//!   the split assignments, indented to the masgn's own column).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, NodeInfo, OptionError,
    OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::MultiWriteNode;
use ruby_ast::{each_descendant, ext, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not use parallel assignment.";

/// Check for simple usages of parallel assignment. It will only warn when
/// the number of variables matches on both sides of the assignment.
#[derive(Debug, Clone)]
pub struct ParallelAssignment {
    indentation_width: u32,
    /// Spans of masgn nodes already reported this file, outermost first --
    /// RuboCop's `ignore_node`/`part_of_ignored_node?`: a masgn nested
    /// inside an already-reported one's RHS (e.g. a lambda literal) is
    /// never itself reported.
    reported_spans: Vec<Span>,
}

impl Rule for ParallelAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Style/ParallelAssignment",
        department: Department::Style,
        summary: "Check for simple usages of parallel assignment. It will only warn when the number of variables matches on both sides of the assignment.",
        explanation: "\
This will only complain when the number of variables being assigned matched \
the number of assigning variables.

```ruby
# bad
a, b, c = 1, 2, 3
a, b, c = [1, 2, 3]

# good
one, two = *foo
a, b = foo
a, b = b, a

a = 1
b = 2
c = 3
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::MultiWriteNode],
        config: &[],
        blind_spots: "\
`accesses?`/`uses_var?`'s receiver comparisons use exact source-text \
equality (this codebase's established approximation for `Parser::AST::Node#==`, \
see `lint/self_assignment.rs`) rather than true structural equality: two \
differently-formatted-but-equal receiver expressions (extra parens, \
different whitespace) would be treated as distinct and so never reordered \
relative to each other, matching every fixture but not upstream's generic \
node equality in the abstract.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        Ok(Self {
            indentation_width: u32::try_from(indentation_width.max(0)).unwrap_or(2),
            reported_spans: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.reported_spans.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(masgn) = node.as_multi_write_node() else { return };
        self.check(node, &masgn, ctx);
    }
}

impl ParallelAssignment {
    fn check(&mut self, node: &Node<'_>, masgn: &MultiWriteNode<'_>, ctx: &mut Context<'_>) {
        // RuboCop's `part_of_ignored_node?`.
        let node_span = node.span();
        if self.reported_spans.iter().any(|s| s.start <= node_span.start && node_span.end <= s.end)
        {
            return;
        }

        // RuboCop's `rhs = node.rhs; rhs = rhs.body if rhs.rescue_type?`.
        let value = masgn.value();
        let (rhs, rescue_expression) = match value.as_rescue_modifier_node() {
            Some(r) => (r.expression(), Some(r.rescue_expression())),
            None => (value, None),
        };

        // `allowed_rhs?`: a non-array RHS, or one containing a splat, is
        // always allowed (see module docs on `Array(rhs).compact`).
        let Some(rhs_array) = rhs.as_array_node() else { return };
        let rhs_elements: Vec<Node<'_>> = rhs_array.elements().iter().collect();
        if rhs_elements.iter().any(|e| e.kind() == NodeKind::SplatNode) {
            return;
        }

        // `allowed_lhs?`: one variable (with a trailing comma), or any
        // anonymous splat.
        let mut lhs_targets = Vec::new();
        flatten_targets(masgn.lefts().iter(), &mut lhs_targets);
        if let Some(rest) = masgn.rest() {
            flatten_targets(std::iter::once(rest), &mut lhs_targets);
        }
        flatten_targets(masgn.rights().iter(), &mut lhs_targets);
        if lhs_targets.len() == 1 || lhs_targets.iter().any(|t| t.kind() == NodeKind::SplatNode) {
            return;
        }

        // `allowed_masign?`'s size check.
        if lhs_targets.len() != rhs_elements.len() {
            return;
        }

        // `contains_heredoc?`.
        if ext::is_heredoc(&rhs) {
            return;
        }
        let mut heredoc = false;
        each_descendant(&rhs, &mut |d| heredoc = heredoc || ext::is_heredoc(d));
        if heredoc {
            return;
        }

        let pairs: Vec<(Node<'_>, Node<'_>)> = lhs_targets.into_iter().zip(rhs_elements).collect();
        let Some(order) = find_valid_order(ctx, &pairs) else { return };

        let span = Span::new(node.span().start, rhs.span().end);
        self.reported_spans.push(node_span);
        let fix = self.build_fix(node, &order, rescue_expression, ctx);
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }

    fn build_fix(
        &self,
        node: &Node<'_>,
        order: &[(Node<'_>, Node<'_>)],
        rescue_expression: Option<Node<'_>>,
        ctx: &Context<'_>,
    ) -> Fix {
        let node_span = node.span();
        let offset = spaces(ctx.line_col(node_span.start).column);

        if let Some(rescue_expression) = rescue_expression {
            let text = self.rescue_correction(node, order, &rescue_expression, ctx, &offset);
            return Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(node_span, text.into_bytes())],
            };
        }

        if let Some(modifier) = modifier_ancestor(ctx.ancestors(), node_span) {
            let text = self.modifier_correction(node, order, modifier.span, ctx, &offset);
            return Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(modifier.span, text.into_bytes())],
            };
        }

        let assignment = Self::join_assignments(order, ctx, &offset);
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node_span, assignment.into_bytes())],
        }
    }

    /// RuboCop's `GenericCorrector#assignment`, joined with `#{offset(node)}`
    /// between entries.
    fn join_assignments(order: &[(Node<'_>, Node<'_>)], ctx: &Context<'_>, offset: &str) -> String {
        let mut out = String::new();
        for (i, (lhs, rhs)) in order.iter().enumerate() {
            if i > 0 {
                out.push('\n');
                out.push_str(offset);
            }
            out.push_str(&String::from_utf8_lossy(ctx.text(lhs.span())));
            out.push_str(" = ");
            out.push_str(&rhs_source(rhs, ctx));
        }
        out
    }

    /// RuboCop's `RescueCorrector#correction`.
    fn rescue_correction(
        &self,
        node: &Node<'_>,
        order: &[(Node<'_>, Node<'_>)],
        rescue_expression: &Node<'_>,
        ctx: &Context<'_>,
        offset: &str,
    ) -> String {
        let indent = format!("{offset}{}", spaces(self.indentation_width));
        if sole_def_body_statement(ctx.ancestors(), node.span()) {
            let assignment = Self::join_assignments(order, ctx, offset);
            format!(
                "{assignment}\nrescue\n{offset}{}",
                String::from_utf8_lossy(ctx.text(rescue_expression.span()))
            )
        } else {
            let assignment = Self::join_assignments(order, ctx, &indent);
            format!(
                "begin\n{indent}{assignment}\n{offset}rescue\n{indent}{}\n{offset}end",
                String::from_utf8_lossy(ctx.text(rescue_expression.span()))
            )
        }
    }

    /// RuboCop's `ModifierCorrector#correction`.
    fn modifier_correction(
        &self,
        node: &Node<'_>,
        order: &[(Node<'_>, Node<'_>)],
        modifier_span: Span,
        ctx: &Context<'_>,
        offset: &str,
    ) -> String {
        let indent = format!("{offset}{}", spaces(self.indentation_width));
        let gap = ctx.text(Span::new(node.span().end, modifier_span.end));
        let skip = gap.iter().take_while(|b| b.is_ascii_whitespace()).count();
        let modifier_text = String::from_utf8_lossy(&gap[skip..]);
        let assignment = Self::join_assignments(order, ctx, &indent);
        format!("{modifier_text}\n{indent}{assignment}\n{offset}end")
    }
}

/// `n` spaces.
fn spaces(n: u32) -> String {
    " ".repeat(n as usize)
}

/// RuboCop's `GenericCorrector#source`: the text to use for one RHS element
/// in the split-out assignment. A `%w(...)` element (a `StringNode` with no
/// `opening_loc`) must be re-quoted as a single-quoted string literal
/// (escaping `\` and `'`); a `%i(...)` element (likewise opening-less
/// `SymbolNode`) must be rendered the way `Symbol#inspect` would. Every
/// other node (including `__FILE__`, a distinct `SourceFileNode` in Prism,
/// never a `StringNode`) uses its own source text verbatim.
fn rhs_source(node: &Node<'_>, ctx: &Context<'_>) -> String {
    if let Some(s) = node.as_string_node() {
        if s.opening_loc().is_none() {
            return quote_single(s.unescaped());
        }
    } else if let Some(s) = node.as_symbol_node() {
        if s.opening_loc().is_none() {
            return inspect_symbol(s.unescaped());
        }
    }
    String::from_utf8_lossy(ctx.text(node.span())).into_owned()
}

/// RuboCop's `GenericCorrector#quote`.
fn quote_single(bytes: &[u8]) -> String {
    let mut out = String::from("'");
    for &b in bytes {
        if b == b'\\' || b == b'\'' {
            out.push('\\');
        }
        out.push(b as char);
    }
    out.push('\'');
    out
}

/// Ruby's `Symbol#inspect`: a plain `:name` when `name` is a valid bare
/// identifier/keyword-ish symbol or one of the operator method names,
/// otherwise a double-quoted `:"name"` (escaping `\` and `"`).
fn inspect_symbol(bytes: &[u8]) -> String {
    if !symbol_needs_quoting(bytes) {
        return format!(":{}", String::from_utf8_lossy(bytes));
    }
    let mut out = String::from(":\"");
    for &b in bytes {
        if b == b'\\' || b == b'"' {
            out.push('\\');
        }
        out.push(b as char);
    }
    out.push('"');
    out
}

const OPERATOR_METHOD_SYMBOLS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

fn symbol_needs_quoting(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    if OPERATOR_METHOD_SYMBOLS.contains(&bytes) {
        return false;
    }
    let mut iter = bytes.iter();
    let Some(&first) = iter.next() else { return true };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return true;
    }
    let rest = &bytes[1..];
    let body = match rest.last() {
        Some(b'?' | b'!' | b'=') => &rest[..rest.len() - 1],
        _ => rest,
    };
    !body.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

/// RuboCop's `MlhsNode#assignments`: flattens `lefts`/`rest`/`rights` (or,
/// recursively, a nested `MultiTargetNode`'s own triad), unwrapping a named
/// splat to its inner target but keeping an anonymous one as the splat node
/// itself.
fn flatten_targets<'pr>(nodes: impl Iterator<Item = Node<'pr>>, out: &mut Vec<Node<'pr>>) {
    for node in nodes {
        match node.kind() {
            // Prism's explicit marker for a bare trailing comma (`a, =`);
            // whitequark has no node for this at all, so it must not count
            // as a real target.
            NodeKind::ImplicitRestNode => {}
            NodeKind::SplatNode => match node.as_splat_node().and_then(|s| s.expression()) {
                Some(inner) => out.push(inner),
                None => out.push(node),
            },
            NodeKind::MultiTargetNode => {
                let Some(mt) = node.as_multi_target_node() else { continue };
                flatten_targets(mt.lefts().iter(), out);
                if let Some(rest) = mt.rest() {
                    flatten_targets(std::iter::once(rest), out);
                }
                flatten_targets(mt.rights().iter(), out);
            }
            _ => out.push(node),
        }
    }
}

/// RuboCop's `var_name` node-pattern (`{(casgn _ $_) (_ $_)}`): the assigned
/// name for a target whose whitequark equivalent has exactly one
/// (`lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`) or two (`casgn`, scope + name)
/// children. A `CallNode` (attribute/index writer) or anonymous splat never
/// matches.
fn target_var_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    match node.kind() {
        NodeKind::LocalVariableTargetNode => {
            Some(node.as_local_variable_target_node()?.name().as_slice())
        }
        NodeKind::InstanceVariableTargetNode => {
            Some(node.as_instance_variable_target_node()?.name().as_slice())
        }
        NodeKind::ClassVariableTargetNode => {
            Some(node.as_class_variable_target_node()?.name().as_slice())
        }
        NodeKind::GlobalVariableTargetNode => {
            Some(node.as_global_variable_target_node()?.name().as_slice())
        }
        NodeKind::ConstantTargetNode => Some(node.as_constant_target_node()?.name().as_slice()),
        NodeKind::ConstantPathTargetNode => {
            Some(node.as_constant_path_target_node()?.name()?.as_slice())
        }
        _ => None,
    }
}

/// RuboCop's `uses_var?` node-search (`{(lvar ivar cvar gvar) %} (const _ %)`),
/// applied to `subtree` and every descendant.
fn uses_var(subtree: &Node<'_>, name: &[u8]) -> bool {
    if reads_var(subtree, name) {
        return true;
    }
    let mut found = false;
    each_descendant(subtree, &mut |d| found = found || reads_var(d, name));
    found
}

fn reads_var(node: &Node<'_>, name: &[u8]) -> bool {
    match node.kind() {
        NodeKind::LocalVariableReadNode => {
            node.as_local_variable_read_node().is_some_and(|n| n.name().as_slice() == name)
        }
        NodeKind::InstanceVariableReadNode => {
            node.as_instance_variable_read_node().is_some_and(|n| n.name().as_slice() == name)
        }
        NodeKind::ClassVariableReadNode => {
            node.as_class_variable_read_node().is_some_and(|n| n.name().as_slice() == name)
        }
        NodeKind::GlobalVariableReadNode => {
            node.as_global_variable_read_node().is_some_and(|n| n.name().as_slice() == name)
        }
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|n| n.name().as_slice() == name)
        }
        NodeKind::ConstantPathNode => node
            .as_constant_path_node()
            .is_some_and(|n| n.name().is_some_and(|nm| nm.as_slice() == name)),
        _ => false,
    }
}

/// RuboCop's `AssignmentSorter#accesses?`'s attribute-writer branch, folding
/// `add_self_to_getters`'s substitution into the receiver comparison
/// directly (see module docs): a candidate call matches when its receiver's
/// source text equals the LHS receiver's, or -- for a bare, argument-less,
/// receiver-less call -- when the LHS receiver is literally `self`.
fn call_matches_getter(
    ctx: &Context<'_>,
    candidate: &Node<'_>,
    access_method: &[u8],
    lhs_receiver: &Node<'_>,
) -> bool {
    let Some(call) = candidate.as_call_node() else { return false };
    if call.is_safe_navigation() || call.name().as_slice() != access_method {
        return false;
    }
    match call.receiver() {
        Some(receiver) => ctx.text(receiver.span()) == ctx.text(lhs_receiver.span()),
        None => call.arguments().is_none() && lhs_receiver.kind() == NodeKind::SelfNode,
    }
}

fn accesses_attr(
    ctx: &Context<'_>,
    rhs: &Node<'_>,
    access_method: &[u8],
    lhs_receiver: &Node<'_>,
) -> bool {
    if call_matches_getter(ctx, rhs, access_method, lhs_receiver) {
        return true;
    }
    let mut found = false;
    each_descendant(rhs, &mut |d| {
        found = found || call_matches_getter(ctx, d, access_method, lhs_receiver);
    });
    found
}

/// RuboCop's `AssignmentSorter#accesses?`'s `lhs.method?(:[]=)` branch: a
/// candidate `[]` read call with the same receiver (by source text) and the
/// same argument list (by source text, pairwise) as the `[]=` write target.
fn call_matches_index(
    ctx: &Context<'_>,
    candidate: &Node<'_>,
    lhs_receiver: &Node<'_>,
    lhs_args: &[Node<'_>],
) -> bool {
    let Some(call) = candidate.as_call_node() else { return false };
    if call.is_safe_navigation() || call.name().as_slice() != b"[]" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    if ctx.text(receiver.span()) != ctx.text(lhs_receiver.span()) {
        return false;
    }
    let args: Vec<Node<'_>> =
        call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
    args.len() == lhs_args.len()
        && args.iter().zip(lhs_args).all(|(a, b)| ctx.text(a.span()) == ctx.text(b.span()))
}

fn accesses_index(
    ctx: &Context<'_>,
    rhs: &Node<'_>,
    lhs_receiver: &Node<'_>,
    lhs_args: &[Node<'_>],
) -> bool {
    if call_matches_index(ctx, rhs, lhs_receiver, lhs_args) {
        return true;
    }
    let mut found = false;
    each_descendant(rhs, &mut |d| {
        found = found || call_matches_index(ctx, d, lhs_receiver, lhs_args);
    });
    found
}

/// RuboCop's `AssignmentSorter#dependency?`. A masgn LHS attribute-writer
/// (`obj.attr1 = ...`) or index-writer (`ary[0] = ...`) target is,
/// respectively, a `CallTargetNode`/`IndexTargetNode` in Prism -- both
/// `send`-shaped in whitequark, hence both eligible for `accesses?` (see
/// module docs' correction of the earlier assumption that index writes were
/// exempt).
fn dependency(ctx: &Context<'_>, lhs: &Node<'_>, rhs: &Node<'_>) -> bool {
    if let Some(name) = target_var_name(lhs) {
        if uses_var(rhs, name) {
            return true;
        }
    }
    match lhs.kind() {
        NodeKind::CallTargetNode => {
            let Some(call) = lhs.as_call_target_node() else { return false };
            let name = call.name().as_slice();
            let Some(access_method) = name.strip_suffix(b"=") else { return false };
            accesses_attr(ctx, rhs, access_method, &call.receiver())
        }
        NodeKind::IndexTargetNode => {
            let Some(call) = lhs.as_index_target_node() else { return false };
            let lhs_args: Vec<Node<'_>> =
                call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
            accesses_index(ctx, rhs, &call.receiver(), &lhs_args)
        }
        _ => false,
    }
}

/// RuboCop's `AssignmentSorter#tsort` (Kahn's algorithm, `None` on a cyclic
/// dependency), given the original zipped `(target, rhs_element)` pairs in
/// source order.
fn find_valid_order<'pr>(
    ctx: &Context<'_>,
    pairs: &[(Node<'pr>, Node<'pr>)],
) -> Option<Vec<(Node<'pr>, Node<'pr>)>> {
    let n = pairs.len();
    let mut edges: Vec<Vec<usize>> = (0..n)
        .map(|i| (0..n).filter(|&j| j != i && dependency(ctx, &pairs[i].0, &pairs[j].1)).collect())
        .collect();
    let mut remaining: Vec<usize> = (0..n).collect();
    let mut result = Vec::with_capacity(n);
    while !remaining.is_empty() {
        let pos = remaining.iter().position(|&i| edges[i].is_empty())?;
        let matched = remaining.remove(pos);
        result.push(matched);
        for &r in &remaining {
            edges[r].retain(|&x| x != matched);
        }
    }
    Some(result.into_iter().map(|i| pairs[i]).collect())
}

/// Whether `span` (the masgn) is the sole statement of a `def`'s body: its
/// immediate parent is a single-statement `StatementsNode` (its span equal
/// to `span`, since a one-statement `StatementsNode`'s span equals its sole
/// child's) and that `StatementsNode`'s own parent is a `DefNode` directly
/// (no `BeginNode` in between -- this masgn's RHS is a `RescueModifierNode`,
/// not a bodystmt-level rescue, so there is none here regardless).
fn sole_def_body_statement(ancestors: &[NodeInfo], span: Span) -> bool {
    let [.., grandparent, parent] = ancestors else { return false };
    parent.kind == NodeKind::StatementsNode
        && parent.span == span
        && grandparent.kind == NodeKind::DefNode
}

/// Whether `span` (the masgn) is the sole statement wrapped by a
/// modifier-form `if`/`unless`/`while`/`until`, mirroring
/// `redundant_require_statement.rs`'s `modifier_form_ancestor`: the
/// immediate parent is a single-statement `StatementsNode` and its own
/// parent is a conditional/loop node starting at the same offset as `span`
/// (the keyword comes after the body only in modifier form; the
/// `begin...end while/until` post-condition loop shape wraps a `BeginNode`
/// between the two, not this masgn directly, so it never matches here).
fn modifier_ancestor(ancestors: &[NodeInfo], span: Span) -> Option<NodeInfo> {
    let [.., grandparent, parent] = ancestors else { return None };
    if parent.kind != NodeKind::StatementsNode || parent.span != span {
        return None;
    }
    let is_conditional = matches!(
        grandparent.kind,
        NodeKind::IfNode | NodeKind::UnlessNode | NodeKind::WhileNode | NodeKind::UntilNode
    );
    (is_conditional && grandparent.span.start == span.start).then_some(*grandparent)
}

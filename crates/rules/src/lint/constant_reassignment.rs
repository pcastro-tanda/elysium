//! `Lint/ConstantReassignment`, ported from RuboCop's
//! `lib/rubocop/cop/lint/constant_reassignment.rb` plus the
//! `ProjectIndexHelp` mixin it includes.
//!
//! `ProjectIndexHelp#project_index` is always unavailable in this engine (no
//! `rubydex`-backed project index), so the cross-file branch
//! (`report_cross_file_collision`) never fires here -- this cop only ever
//! tracks reassignment within a single file, which is all its own fixtures
//! exercise.
//!
//! Whitequark's `casgn` node type covers both a bare `FOO = x` and a
//! namespaced `A::FOO = x`/`self::FOO = x`/`::FOO = x`; Prism splits these
//! into `ConstantWriteNode` (bare) and `ConstantPathWriteNode` (namespaced,
//! whose `target` is the `ConstantPathNode` holding the dotted chain). A
//! `casgn` embedded in a multiple assignment (`FOO, BAR = 1, 2`) is instead
//! Prism's `ConstantTargetNode`/`ConstantPathTargetNode` -- assignment
//! *targets*, which never carry a `value` field, since (unlike a standalone
//! `casgn`) whitequark's own `masgn`-nested `casgn` doesn't either.
//!
//! Prism has no parent pointers, so the whole cop runs from `file_start` as
//! one recursive descent (see [`Walker::visit`]), threading two stacks down
//! through the recursion instead of looking parents up afterwards: the full
//! ancestor chain (`parent_chain`, for [`is_simple_assignment`]'s climb,
//! which must stop at the *first* enclosing `class`/`module` -- scanned
//! innermost-first, i.e. in reverse push order) and the `class`/`module`
//! ancestors alone, outermost first (`class_module_chain`, already in the
//! right order for [`ancestor_namespaces`] without a separate reversal).
//!
//! `node.loc.operator`/message source slicing (`node.namespace.source` for
//! an absolute `casgn`) is restated here by rebuilding the qualified name
//! from plain segment lists ([`path_segments`]/[`qualify`]) rather than
//! slicing raw source text; the two produce identical strings as long as
//! `fixed_constant_path?` (folded into [`path_segments`] returning `None`
//! for anything but a `const`/`cbase`/`self` path) already rejected anything
//! exotic.

use std::collections::HashSet;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{for_each_child, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// rubocop-ast's `LITERALS` (`TRUTHY_LITERALS + FALSEY_LITERALS`).
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
            | NodeKind::RangeNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
    )
}

/// `Node#each_path.all? { |path| path.type?(:cbase, :const, :self) }`
/// *and* the segment list that walk collects, combined: `None` means the
/// path is not fixed (contains a variable, method call, ...); `Some((
/// is_absolute, segments))` otherwise. `self` segments contribute nothing
/// to `segments` (matching `constant_namespaces`' `select(&:const_type?)`)
/// but do not themselves disqualify the path.
fn path_segments<'pr>(node: &Node<'pr>) -> Option<(bool, Vec<&'pr [u8]>)> {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            Some((false, vec![node.as_constant_read_node()?.name().as_slice()]))
        }
        NodeKind::SelfNode => Some((false, Vec::new())),
        NodeKind::ConstantPathNode => {
            let n = node.as_constant_path_node()?;
            let name = n.name().map(|id| id.as_slice());
            match n.parent() {
                Some(parent) => {
                    let (absolute, mut segments) = path_segments(&parent)?;
                    segments.extend(name);
                    Some((absolute, segments))
                }
                None => Some((true, name.into_iter().collect())),
            }
        }
        NodeKind::ConstantPathTargetNode => {
            let n = node.as_constant_path_target_node()?;
            let name = n.name().map(|id| id.as_slice());
            match n.parent() {
                Some(parent) => {
                    let (absolute, mut segments) = path_segments(&parent)?;
                    segments.extend(name);
                    Some((absolute, segments))
                }
                None => Some((true, name.into_iter().collect())),
            }
        }
        _ => None,
    }
}

/// A `casgn`-like node's shape: `(is_absolute, segments before the final
/// one, final segment)`. `None` for anything [`path_segments`] rejects.
type CasgnShape<'pr> = (bool, Vec<&'pr [u8]>, &'pr [u8]);

fn casgn_shape<'pr>(node: &Node<'pr>) -> Option<CasgnShape<'pr>> {
    match node.kind() {
        NodeKind::ConstantWriteNode => {
            Some((false, Vec::new(), node.as_constant_write_node()?.name().as_slice()))
        }
        NodeKind::ConstantTargetNode => {
            Some((false, Vec::new(), node.as_constant_target_node()?.name().as_slice()))
        }
        NodeKind::ConstantPathWriteNode => {
            let target = node.as_constant_path_write_node()?.target();
            let (absolute, mut segments) = path_segments(&target.as_node())?;
            let final_segment = segments.pop()?;
            Some((absolute, segments, final_segment))
        }
        NodeKind::ConstantPathTargetNode => {
            let (absolute, mut segments) = path_segments(node)?;
            let final_segment = segments.pop()?;
            Some((absolute, segments, final_segment))
        }
        _ => None,
    }
}

fn join_colons(parts: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(b"::");
        }
        out.extend_from_slice(part);
    }
    out
}

/// `fully_qualified_constant_name`/`definition_name`'s shared formula: an
/// absolute path ignores the enclosing namespace entirely; otherwise the
/// enclosing namespace and the path's own leading segments are joined, and
/// the whole thing always gets a leading `::` (`fully_qualified_name_for`'s
/// `['', *namespaces, constant].join('::')`).
fn qualify(
    ancestor_ns: &[Vec<u8>],
    absolute: bool,
    own_prefix: &[&[u8]],
    final_name: &[u8],
) -> Vec<u8> {
    let mut parts: Vec<Vec<u8>> = vec![Vec::new()];
    if !absolute {
        parts.extend(ancestor_ns.iter().cloned());
    }
    parts.extend(own_prefix.iter().map(|s| s.to_vec()));
    parts.push(final_name.to_vec());
    join_colons(&parts)
}

/// `constant_display_name`: the path's own segments only, no leading `::`.
fn display_name(own_prefix: &[&[u8]], final_name: &[u8]) -> Vec<u8> {
    let mut parts: Vec<Vec<u8>> = own_prefix.iter().map(|s| s.to_vec()).collect();
    parts.push(final_name.to_vec());
    join_colons(&parts)
}

/// `ClassNode`/`ModuleNode`'s own name expression.
fn constant_path_of<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    match node.kind() {
        NodeKind::ClassNode => Some(node.as_class_node()?.constant_path()),
        NodeKind::ModuleNode => Some(node.as_module_node()?.constant_path()),
        _ => None,
    }
}

/// `ancestor_namespaces`: folds every `class`/`module` ancestor's own
/// identifier into a running namespace list, outermost first (matching
/// `append_namespaces`' clear-on-absolute behaviour for each).
fn ancestor_namespaces(class_module_chain: &[Node<'_>]) -> Vec<Vec<u8>> {
    let mut namespaces: Vec<Vec<u8>> = Vec::new();
    for ancestor in class_module_chain {
        let Some(identifier) = constant_path_of(ancestor) else { continue };
        let Some((absolute, mut segments)) = path_segments(&identifier) else { continue };
        let Some(final_segment) = segments.pop() else { continue };
        if absolute {
            namespaces.clear();
        }
        namespaces.extend(segments.into_iter().map(<[u8]>::to_vec));
        namespaces.push(final_segment.to_vec());
    }
    namespaces
}

/// `unconditional_definition?`: every ancestor is a plain statement
/// sequence or another `class`/`module` -- no conditional/block wrapping.
fn is_unconditional_definition(parent_chain: &[Node<'_>]) -> bool {
    parent_chain.iter().all(|ancestor| {
        matches!(
            ancestor.kind(),
            NodeKind::StatementsNode
                | NodeKind::ClassNode
                | NodeKind::ModuleNode
                | NodeKind::ProgramNode
        )
    })
}

/// `simple_assignment?`: climbs ancestors innermost-first (the reverse of
/// `parent_chain`'s outermost-first push order), stopping at the first
/// `class`/`module` (acceptable, matching upstream's early `return true`)
/// or the first ancestor outside the acceptable set (not simple).
fn is_simple_assignment(parent_chain: &[Node<'_>]) -> bool {
    for ancestor in parent_chain.iter().rev() {
        match ancestor.kind() {
            NodeKind::ClassNode | NodeKind::ModuleNode => return true,
            NodeKind::StatementsNode
            | NodeKind::ProgramNode
            | NodeKind::MultiWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ConstantTargetNode
            | NodeKind::ConstantPathTargetNode => {}
            k if is_literal_kind(k) => {}
            NodeKind::CallNode
                if ancestor.as_call_node().is_some_and(|c| c.name().as_slice() == b"freeze") => {}
            _ => return false,
        }
    }
    true
}

/// `remove_constant`: `(send {nil? self} :remove_const ({sym str} $_))`,
/// `csend` excluded (upstream never aliases `on_csend` for this cop).
fn remove_const_target(node: &Node<'_>) -> Option<Vec<u8>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() || call.name().as_slice() != b"remove_const" {
        return None;
    }
    match call.receiver() {
        None => {}
        Some(receiver) if receiver.kind() == NodeKind::SelfNode => {}
        Some(_) => return None,
    }
    let list = call.arguments()?.arguments();
    if list.len() != 1 {
        return None;
    }
    match list.last()?.kind() {
        NodeKind::StringNode => Some(list.last()?.as_string_node()?.unescaped().to_vec()),
        NodeKind::SymbolNode => Some(list.last()?.as_symbol_node()?.unescaped().to_vec()),
        _ => None,
    }
}

struct Walker<'pr> {
    definitions: HashSet<Vec<u8>>,
    offenses: Vec<(Span, String)>,
    parent_chain: Vec<Node<'pr>>,
    class_module_chain: Vec<Node<'pr>>,
}

impl<'pr> Walker<'pr> {
    fn new() -> Self {
        Self {
            definitions: HashSet::new(),
            offenses: Vec::new(),
            parent_chain: Vec::new(),
            class_module_chain: Vec::new(),
        }
    }

    fn visit(&mut self, node: Node<'pr>) {
        match node.kind() {
            NodeKind::ClassNode | NodeKind::ModuleNode => self.visit_definition(&node),
            NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ConstantTargetNode
            | NodeKind::ConstantPathTargetNode => self.visit_casgn(&node),
            NodeKind::CallNode => self.visit_remove_const(&node),
            _ => {}
        }

        let is_class_or_module = matches!(node.kind(), NodeKind::ClassNode | NodeKind::ModuleNode);
        self.parent_chain.push(node);
        if is_class_or_module {
            self.class_module_chain.push(node);
        }

        let mut children = Vec::new();
        for_each_child(&node, |child| children.push(*child));
        for child in children {
            self.visit(child);
        }

        self.parent_chain.pop();
        if is_class_or_module {
            self.class_module_chain.pop();
        }
    }

    fn visit_definition(&mut self, node: &Node<'pr>) {
        if !is_unconditional_definition(&self.parent_chain) {
            return;
        }
        let Some(identifier) = constant_path_of(node) else { return };
        let Some((absolute, mut segments)) = path_segments(&identifier) else { return };
        let Some(final_segment) = segments.pop() else { return };
        let ancestor_ns = ancestor_namespaces(&self.class_module_chain);
        let key = qualify(&ancestor_ns, absolute, &segments, final_segment);
        self.definitions.insert(key);
    }

    fn visit_casgn(&mut self, node: &Node<'pr>) {
        let Some((absolute, own_prefix, final_name)) = casgn_shape(node) else { return };
        if !is_simple_assignment(&self.parent_chain) {
            return;
        }
        let ancestor_ns = ancestor_namespaces(&self.class_module_chain);
        let key = qualify(&ancestor_ns, absolute, &own_prefix, final_name);

        if self.definitions.contains(&key) {
            let display =
                String::from_utf8_lossy(&display_name(&own_prefix, final_name)).into_owned();
            self.offenses.push((
                node.span(),
                format!("Constant `{display}` is already assigned in this namespace."),
            ));
        } else {
            self.definitions.insert(key);
        }
    }

    fn visit_remove_const(&mut self, node: &Node<'pr>) {
        let Some(constant) = remove_const_target(node) else { return };
        let ancestor_ns = ancestor_namespaces(&self.class_module_chain);
        if ancestor_ns.is_empty() {
            return;
        }
        let key = qualify(&ancestor_ns, false, &[], &constant);
        self.definitions.remove(&key);
    }
}

/// Checks for constant reassignments.
#[derive(Debug, Clone)]
pub struct ConstantReassignment;

impl Rule for ConstantReassignment {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ConstantReassignment",
        department: Department::Lint,
        summary: "Checks for constant reassignments.",
        explanation: "\
Checks for constant reassignments.

Emulates Ruby's runtime warning \"already initialized constant X\" when a \
constant is reassigned in the same file and namespace.

The cop tracks constants defined via `NAME = value` syntax as well as \
class/module keyword definitions. It detects reassignment when a constant \
is first defined one way and then redefined using the `NAME = value` \
syntax.

The cop cannot catch all offenses, like, for example, when using \
metaprogramming (`Module#const_set`).

The cop cannot detect reassignment across files.

The cop only takes into account constants assigned in a \"simple\" way: \
directly inside class/module definition, or within another constant. Other \
type of assignments (e.g., inside a conditional) are disregarded.

The cop also tracks constant removal using `Module#remove_const` with \
symbol or string argument.

```ruby
# bad
X = :foo
X = :bar

# bad
class A
  X = :foo
  X = :bar
end

# bad
module A
  X = :foo
  X = :bar
end

# bad
class FooError < StandardError; end
FooError = Class.new(RuntimeError)

# bad
module M; end
M = 1

# good - keep only one assignment
X = :bar

class A
  X = :bar
end

module A
  X = :bar
end

# good - use OR assignment
X = :foo
X ||= :bar

# good - use conditional assignment
X = :foo
X = :bar unless defined?(X)

# good - remove the assigned constant first
class A
  X = :foo
  remove_const :X
  X = :bar
end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots:
            "No project-wide index is available, so reassignment of a constant previously \
defined in another file is never detected (upstream's `AllCops/UseProjectIndex` cross-file branch).",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let mut walker = Walker::new();
        walker.visit(root);

        for (span, message) in walker.offenses {
            ctx.report(&Self::META, span, message);
        }
    }
}

//! rubocop-ast's `Node#parent_module_name`, shared by the cops that need it.
//!
//! In whitequark a `block` node wraps the `send` it is attached to, so a
//! call with a literal block is an ancestor of its own receiver, arguments
//! and body (and of itself); Prism's `CallNode` already is that node, which
//! is why every call with a literal block gets a frame.

use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// The node kinds a rule must subscribe to for [`ParentModule`].
pub const KINDS: &[NodeKind] = &[
    NodeKind::ClassNode,
    NodeKind::ModuleNode,
    NodeKind::SingletonClassNode,
    NodeKind::ConstantWriteNode,
    NodeKind::ConstantPathWriteNode,
    NodeKind::LambdaNode,
    NodeKind::CallNode,
];

/// What one ancestor contributes to `parent_module_name`.
#[derive(Debug, Clone)]
enum Part {
    /// A module name segment.
    Name(String),
    /// Ignored (`filter_map` drops it).
    Skip,
    /// The name cannot be determined (`return nil`).
    Poison,
}

/// One entered node that can be an ancestor in `parent_module_name`.
#[derive(Debug, Clone)]
struct Frame {
    part: Part,
    /// For a constant assignment, the span of its value, so that a `Class.new`
    /// block can tell it is the assignment's direct value.
    casgn_value: Option<Span>,
}

/// The stack of entered nodes that can be ancestors in `parent_module_name`.
#[derive(Debug, Clone, Default)]
pub struct ParentModule {
    frames: Vec<Frame>,
}

impl ParentModule {
    /// Records `node`, entered by the rule; call from `enter` for every node
    /// of [`KINDS`] (after reading [`Self::name`] if the node itself is the
    /// subject, since a node is not its own ancestor).
    pub fn enter(&mut self, node: &Node<'_>) {
        let frame = self.frame_for(node);
        self.frames.push(frame);
    }

    /// Pops the node being left.
    pub fn leave(&mut self) {
        self.frames.pop();
    }

    fn frame_for(&self, node: &Node<'_>) -> Frame {
        let mut casgn_value = None;
        let part = match node.kind() {
            NodeKind::ClassNode => node
                .as_class_node()
                .and_then(|c| const_name(&c.constant_path()))
                .map_or(Part::Skip, Part::Name),
            NodeKind::ModuleNode => node
                .as_module_node()
                .and_then(|m| const_name(&m.constant_path()))
                .map_or(Part::Skip, Part::Name),
            NodeKind::SingletonClassNode => match node.as_singleton_class_node() {
                Some(sclass) => {
                    let subject = sclass.expression();
                    match subject.kind() {
                        NodeKind::ConstantReadNode | NodeKind::ConstantPathNode => {
                            const_name(&subject)
                                .map_or(Part::Poison, |n| Part::Name(format!("#<Class:{n}>")))
                        }
                        NodeKind::SelfNode => {
                            Part::Name(format!("#<Class:{}>", self.name().unwrap_or_default()))
                        }
                        _ => Part::Poison,
                    }
                }
                None => Part::Skip,
            },
            NodeKind::ConstantWriteNode => match node.as_constant_write_node() {
                Some(write) => {
                    casgn_value = Some(write.value().span());
                    if is_class_new(&write.value()) {
                        Part::Name(String::from_utf8_lossy(write.name().as_slice()).into_owned())
                    } else {
                        Part::Skip
                    }
                }
                None => Part::Skip,
            },
            NodeKind::ConstantPathWriteNode => match node.as_constant_path_write_node() {
                Some(write) => {
                    casgn_value = Some(write.value().span());
                    match const_name(&write.target().as_node()) {
                        Some(name) if is_class_new(&write.value()) => Part::Name(name),
                        _ => Part::Skip,
                    }
                }
                None => Part::Skip,
            },
            NodeKind::LambdaNode => match node.as_lambda_node() {
                Some(lambda) if !uses_implicit_params(lambda.parameters()) => Part::Poison,
                _ => Part::Skip,
            },
            _ => node.as_call_node().map_or(Part::Skip, |call| self.call_block_part(node, &call)),
        };
        Frame { part, casgn_value }
    }

    /// `parent_module_name_part` for a `block` ancestor.
    fn call_block_part(&self, node: &Node<'_>, call: &CallNode<'_>) -> Part {
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else {
            return Part::Skip;
        };
        // `numblock`/`itblock` are not `:block`.
        if uses_implicit_params(block.parameters()) {
            return Part::Skip;
        }
        if call.name().as_slice() == b"class_eval" {
            return match call.receiver() {
                None => Part::Skip,
                Some(receiver) => match receiver.kind() {
                    NodeKind::ConstantReadNode | NodeKind::ConstantPathNode => {
                        const_name(&receiver).map_or(Part::Poison, Part::Name)
                    }
                    _ => Part::Poison,
                },
            };
        }
        // `new_class_or_module_block?`: `^(casgn _ _ (block (send (const _
        // {:Class :Module}) :new) ...))`.
        let direct_value =
            self.frames.last().and_then(|f| f.casgn_value).is_some_and(|span| span == node.span());
        if direct_value && is_class_or_module_new_without_args(call) {
            Part::Skip
        } else {
            Part::Poison
        }
    }

    /// `Node#parent_module_name` of the node being entered: `None` when it
    /// cannot be determined.
    pub fn name(&self) -> Option<String> {
        let mut names = Vec::new();
        for frame in &self.frames {
            match &frame.part {
                Part::Name(name) => names.push(name.as_str()),
                Part::Skip => {}
                Part::Poison => return None,
            }
        }
        if names.is_empty() {
            Some("Object".to_string())
        } else {
            Some(names.join("::"))
        }
    }
}

/// Numbered (`_1`) or `it` block parameters, which whitequark models as
/// `numblock`/`itblock` rather than `block`.
fn uses_implicit_params(parameters: Option<Node<'_>>) -> bool {
    parameters.is_some_and(|p| {
        matches!(p.kind(), NodeKind::NumberedParametersNode | NodeKind::ItParametersNode)
    })
}

/// `(send #global_const?({:Class :Module}) :new ...)`.
fn is_class_new(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.name().as_slice() != b"new" || call.is_safe_navigation() {
        return false;
    }
    call.receiver().is_some_and(|r| {
        is_bare_or_toplevel_const(&r)
            && matches!(const_name(&r).as_deref(), Some("Class" | "Module"))
    })
}

/// `(send (const _ {:Class :Module}) :new)`.
fn is_class_or_module_new_without_args(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"new" || call.is_safe_navigation() || call.arguments().is_some() {
        return false;
    }
    call.receiver().is_some_and(|r| match r.kind() {
        NodeKind::ConstantReadNode => r
            .as_constant_read_node()
            .is_some_and(|c| matches!(c.name().as_slice(), b"Class" | b"Module")),
        NodeKind::ConstantPathNode => r.as_constant_path_node().is_some_and(|c| {
            c.name().is_some_and(|n| matches!(n.as_slice(), b"Class" | b"Module"))
        }),
        _ => false,
    })
}

//! `Rails/RedundantForeignKey`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/redundant_foreign_key.rb`.
//!
//! The default foreign key of `has_one`/`has_many`/`has_and_belongs_to_many`
//! comes from rubocop-ast's `Node#parent_module_name` (this module's
//! [`Frame`] stack models the ancestors it inspects) fed through
//! ActiveSupport's `String#foreign_key`.
//!
//! In whitequark a `block` node wraps the `send` it is attached to, so a
//! call with a literal block is an ancestor of its own receiver, arguments
//! and body (and of itself, for the association call); Prism's `CallNode`
//! already is that node, which is why every call with a literal block gets
//! a frame and the association's own frame counts.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

const MSG: &str = "Specifying the default value for `foreign_key` is redundant.";

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

/// Checks for associations where the `:foreign_key` option is redundant.
#[derive(Debug, Clone, Default)]
pub struct RedundantForeignKey {
    frames: Vec<Frame>,
}

impl Rule for RedundantForeignKey {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RedundantForeignKey",
        department: Department::Rails,
        summary: "Checks for associations where the `:foreign_key` option is redundant.",
        explanation: "Detects cases where the `:foreign_key` option on associations is \
                      redundant.\n\n```ruby\n# bad\nclass Post\n  has_many :comments, \
                      foreign_key: 'post_id'\nend\n\nclass Comment\n  belongs_to :post, \
                      foreign_key: 'post_id'\nend\n\n# good\nclass Post\n  has_many \
                      :comments\nend\n\nclass Comment\n  belongs_to :author, foreign_key: \
                      'user_id'\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::ClassNode,
            NodeKind::ModuleNode,
            NodeKind::SingletonClassNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::LambdaNode,
            NodeKind::CallNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let frame = self.frame_for(node);
        self.frames.push(frame);
        if let Some(call) = node.as_call_node() {
            self.check(&call, ctx);
        }
    }

    fn leave(&mut self, _node: &Node<'_>, _ctx: &mut Context<'_>) {
        self.frames.pop();
    }
}

impl RedundantForeignKey {
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
                        NodeKind::SelfNode => Part::Name(format!(
                            "#<Class:{}>",
                            self.parent_module_name().unwrap_or_default()
                        )),
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
        let direct_value = self
            .frames
            .last()
            .and_then(|f| f.casgn_value)
            .is_some_and(|span| span == node.span());
        if direct_value && is_class_or_module_new_without_args(call) {
            Part::Skip
        } else {
            Part::Poison
        }
    }

    /// `Node#parent_module_name` of the node being entered: `None` when it
    /// cannot be determined.
    fn parent_module_name(&self) -> Option<String> {
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

    fn check(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let association = call.name();
        let association = association.as_slice();
        if !matches!(
            association,
            b"belongs_to" | b"has_one" | b"has_many" | b"has_and_belongs_to_many"
        ) || call.receiver().is_some()
            || call.block().is_some_and(|b| b.as_block_argument_node().is_some())
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(name), Some(options), None) = (arguments.next(), arguments.next(), arguments.next())
        else {
            return;
        };
        let Some(name) = sym_or_str(&name) else { return };
        let elements: Vec<Node<'_>> = match options.kind() {
            NodeKind::HashNode => options.as_hash_node().map(|h| h.elements().iter().collect()),
            NodeKind::KeywordHashNode => {
                options.as_keyword_hash_node().map(|h| h.elements().iter().collect())
            }
            _ => None,
        }
        .unwrap_or_default();
        let pairs: Vec<_> = elements.iter().filter_map(|e| e.as_assoc_node()).collect();
        let found = pairs.iter().find_map(|pair| {
            let key = pair.key();
            if !key.as_symbol_node().is_some_and(|s| s.unescaped() == b"foreign_key") {
                return None;
            }
            Some((pair, sym_or_str(&pair.value())?))
        });
        let Some((pair, foreign_key)) = found else { return };

        let default = if association == b"belongs_to" {
            Some(format!("{name}_id"))
        } else if let Some(as_value) = find_as_option(&pairs) {
            Some(format!("{as_value}_id"))
        } else {
            self.parent_module_name().map(|module| foreign_key_of(&module))
        };
        if default.as_deref() != Some(foreign_key.as_str()) {
            return;
        }

        let pair_span = pair.as_node().span();
        let mut range = ctx.with_surrounding_space(pair_span, Side::Left, true, false);
        let bytes = ctx.source().bytes();
        let mut begin = range.start as usize;
        while begin > 0 && bytes[begin - 1] == b',' {
            begin -= 1;
        }
        range = Span::new(u32::try_from(begin).expect("offset exceeds u32"), range.end);
        ctx.report_with_fix(
            &Self::META,
            pair_span,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] },
        );
    }
}

/// `({sym str} $_)`: the value of a symbol or plain string literal.
fn sym_or_str(node: &Node<'_>) -> Option<String> {
    let bytes = match node.kind() {
        NodeKind::SymbolNode => node.as_symbol_node()?.unescaped().to_vec(),
        NodeKind::StringNode => node.as_string_node()?.unescaped().to_vec(),
        _ => return None,
    };
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// `find_as_option`: the `as:` value when it is a literal.
fn find_as_option(pairs: &[ruby_ast::node::AssocNode<'_>]) -> Option<String> {
    pairs
        .iter()
        .find(|pair| pair.key().as_symbol_node().is_some_and(|s| s.unescaped() == b"as"))
        .and_then(|pair| sym_or_str(&pair.value()))
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
        is_bare_or_toplevel_const(&r) && matches!(const_name(&r).as_deref(), Some("Class" | "Module"))
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

/// ActiveSupport's `String#foreign_key`: `underscore(demodulize(name)) + "_id"`.
fn foreign_key_of(class_name: &str) -> String {
    let demodulized = class_name.rsplit("::").next().unwrap_or(class_name);
    let chars: Vec<char> = demodulized.chars().collect();
    let mut out = String::with_capacity(chars.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let prev = chars[i - 1];
            let after_upper_run = (prev.is_ascii_uppercase() || prev.is_ascii_digit())
                && chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
            let after_lower = prev.is_ascii_lowercase() || prev.is_ascii_digit();
            if after_upper_run || after_lower {
                out.push('_');
            }
        }
        out.push(if c == '-' { '_' } else { c });
    }
    let mut key = out.to_lowercase();
    key.push_str("_id");
    key
}

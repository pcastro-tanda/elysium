//! `ThreadSafety/ClassAndModuleAttributes`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/class_and_module_attributes.rb`.
//!
//! `defined_in_singleton_class?` walks `node.ancestors`, where a `def` stops
//! the search but a `defs` (`def self.x`) does not; the rule keeps its own
//! ancestor stack because the engine's ancestor info does not distinguish them.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{walk, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

const MSG: &str = "Avoid mutating class and module attributes.";

/// Avoid mutating class and module attributes.
#[derive(Debug, Clone)]
pub struct ClassAndModuleAttributes {
    class_attribute_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anc {
    Def,
    Sclass,
    Other,
}

struct Collector {
    class_attribute_allowed: bool,
    stack: Vec<Anc>,
    offenses: Vec<Span>,
}

impl Collector {
    fn defined_in_singleton_class(&self) -> bool {
        for anc in self.stack.iter().rev() {
            match anc {
                Anc::Def => return false,
                Anc::Sclass => return true,
                Anc::Other => {}
            }
        }
        false
    }
}

impl<'pr> Visitor<'pr> for Collector {
    fn enter(&mut self, node: &Node<'pr>) {
        if let Some(call) = node.as_call_node() {
            if !call.is_safe_navigation() && call.receiver().is_none() {
                let hit = match call.name().as_slice() {
                    b"mattr_writer" | b"mattr_accessor" | b"cattr_writer" | b"cattr_accessor" => {
                        true
                    }
                    b"class_attribute" => !self.class_attribute_allowed,
                    b"attr"
                    | b"attr_accessor"
                    | b"attr_writer"
                    | b"attr_internal"
                    | b"attr_internal_accessor"
                    | b"attr_internal_writer" => self.defined_in_singleton_class(),
                    _ => false,
                };
                if hit {
                    self.offenses.push(call_span_excluding_block(&call));
                }
            }
        }
        let anc = match node.kind() {
            NodeKind::DefNode if node.as_def_node().is_some_and(|d| d.receiver().is_none()) => {
                Anc::Def
            }
            NodeKind::SingletonClassNode => Anc::Sclass,
            _ => Anc::Other,
        };
        self.stack.push(anc);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

impl Rule for ClassAndModuleAttributes {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/ClassAndModuleAttributes",
        department: Department::ThreadSafety,
        summary: "Avoid mutating class and module attributes.",
        explanation: "Avoid mutating class and module attributes. They are implemented by class \
                      variables, which are not thread-safe.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[ConfigOption {
            name: "ActiveSupportClassAttributeAllowed",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Allow `class_attribute` (ActiveSupport).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { class_attribute_allowed: options.bool("ActiveSupportClassAttributeAllowed") })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let mut collector = Collector {
            class_attribute_allowed: self.class_attribute_allowed,
            stack: Vec::new(),
            offenses: Vec::new(),
        };
        walk(&ctx.parsed().root(), &mut collector);
        for span in collector.offenses {
            ctx.report(&Self::META, span, MSG);
        }
    }
}

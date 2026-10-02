//! `Rails/LexicallyScopedActionFilter`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/lexically_scoped_action_filter.rb`.

use std::collections::HashMap;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const FILTERS: &[&[u8]] = &[
    b"after_action",
    b"append_after_action",
    b"append_around_action",
    b"append_before_action",
    b"around_action",
    b"before_action",
    b"prepend_after_action",
    b"prepend_around_action",
    b"prepend_before_action",
    b"skip_after_action",
    b"skip_around_action",
    b"skip_before_action",
    b"skip_action_callback",
];

/// A class or module: its span, whether it is a class, and the action
/// methods its body defines.
#[derive(Debug, Clone)]
struct Scope {
    span: Span,
    is_class: bool,
    defined: Vec<String>,
}

/// Checks that methods specified in the filter's `only` or `except` options
/// are defined within the same class or module.
#[derive(Debug, Clone)]
pub struct LexicallyScopedActionFilter {
    /// Every class/module entered so far, outermost first.
    scopes: Vec<Scope>,
}

impl Rule for LexicallyScopedActionFilter {
    const META: RuleMeta = RuleMeta {
        name: "Rails/LexicallyScopedActionFilter",
        department: Department::Rails,
        summary: "Checks that methods specified in the filter's `only` or `except` options are defined within the same class or module.",
        explanation: "Checks that methods specified in the filter's `only` or `except` options \
                      are defined within the same class or module.\n\nYou can technically \
                      specify methods of superclass or methods added by mixins on the filter, \
                      but these can confuse developers. If you specify methods that are defined \
                      in other classes or modules, you should define the filter in that class or \
                      module.\n\n```ruby\n# bad\nclass LoginController < ApplicationController\n  \
                      before_action :require_login, only: %i[index settings logout]\n\n  def \
                      index\n  end\nend\n\n# good\nclass LoginController < ApplicationController\n  \
                      before_action :require_login, only: %i[index settings logout]\n\n  def \
                      index\n  end\n\n  def settings\n  end\n\n  def logout\n  end\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ClassNode, NodeKind::ModuleNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { scopes: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ClassNode => {
                let Some(class) = node.as_class_node() else { return };
                self.scopes.push(Scope {
                    span: node.span(),
                    is_class: true,
                    defined: defined_action_methods(class.body().as_ref()),
                });
            }
            NodeKind::ModuleNode => {
                let Some(module) = node.as_module_node() else { return };
                self.scopes.push(Scope {
                    span: node.span(),
                    is_class: false,
                    defined: defined_action_methods(module.body().as_ref()),
                });
            }
            NodeKind::CallNode => self.check_filter(node, ctx),
            _ => {}
        }
    }
}

impl LexicallyScopedActionFilter {
    fn check_filter(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_some()
            || call.is_safe_navigation()
            || !FILTERS.contains(&call.name().as_slice())
            || call.block().is_some_and(|b| b.as_block_argument_node().is_some())
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let list: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let [_, options] = &list[..] else { return };
        let elements = if let Some(hash) = options.as_keyword_hash_node() {
            hash.elements()
        } else if let Some(hash) = options.as_hash_node() {
            hash.elements()
        } else {
            return;
        };
        let mut elements = elements.iter();
        let (Some(only), None) = (elements.next(), elements.next()) else { return };
        let Some(pair) = only.as_assoc_node() else { return };
        let Some(key) = pair.key().as_symbol_node() else { return };
        if !matches!(key.unescaped(), b"only" | b"except") {
            return;
        }
        let Some(scope) = self.scopes.iter().rev().find(|scope| contains(scope.span, node.span()))
        else {
            return;
        };
        let unmatched: Vec<String> = array_values(&pair.value())
            .into_iter()
            .filter(|name| !scope.defined.contains(name))
            .collect();
        if unmatched.is_empty() {
            return;
        }
        let kind = if scope.is_class { "class" } else { "module" };
        let action = if unmatched.len() == 1 {
            format!("`{}` is", unmatched[0])
        } else {
            format!("`{}` are", unmatched.join("`, `"))
        };
        ctx.report(
            &Self::META,
            call_span_excluding_block(&call),
            format!("{action} not explicitly defined on the {kind}."),
        );
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// `array_values`: the action names a `only:`/`except:` value lists.
fn array_values(node: &Node<'_>) -> Vec<String> {
    let name = |n: &Node<'_>| {
        n.as_string_node().map(|s| String::from_utf8_lossy(s.unescaped()).into_owned()).or_else(
            || n.as_symbol_node().map(|s| String::from_utf8_lossy(s.unescaped()).into_owned()),
        )
    };
    if let Some(array) = node.as_array_node() {
        array.elements().iter().filter_map(|element| name(&element)).collect()
    } else {
        name(node).into_iter().collect()
    }
}

/// `defined_action_methods(block)`: `block` is the class body's `begin`
/// node, which whitequark builds only for two or more statements.
fn defined_action_methods(body: Option<&Node<'_>>) -> Vec<String> {
    let Some(statements) = body.and_then(Node::as_statements_node) else { return Vec::new() };
    let children: Vec<Node<'_>> = statements.body().iter().collect();
    if children.len() < 2 {
        return Vec::new();
    }
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();

    let defined: Vec<String> = children
        .iter()
        .filter_map(Node::as_def_node)
        .filter(|def| def.receiver().is_none())
        .map(|def| text(def.name().as_slice()))
        .collect();

    let mut delegated = Vec::new();
    let mut alias_of: HashMap<String, String> = HashMap::new();
    for child in &children {
        if let Some(call) = child.as_call_node() {
            if call.block().is_some_and(|b| b.as_block_node().is_some())
                || call.is_safe_navigation()
            {
                continue;
            }
            match call.name().as_slice() {
                b"delegate" if call.receiver().is_none() => {
                    delegated.extend(delegated_methods(&call));
                }
                b"alias_method" => {
                    let Some(arguments) = call.arguments() else { continue };
                    let all_args: Vec<Node<'_>> = arguments.arguments().iter().collect();
                    let (Some(first), Some(final_arg)) = (all_args.first(), all_args.last()) else {
                        continue;
                    };
                    if let (Some(new), Some(old)) =
                        (first.as_symbol_node(), final_arg.as_symbol_node())
                    {
                        alias_of.insert(text(old.unescaped()), text(new.unescaped()));
                    }
                }
                _ => {}
            }
        } else if let Some(alias) = child.as_alias_method_node() {
            if let (Some(new), Some(old)) =
                (alias.new_name().as_symbol_node(), alias.old_name().as_symbol_node())
            {
                alias_of.insert(text(old.unescaped()), text(new.unescaped()));
            }
        }
    }
    let via_alias: Vec<String> =
        defined.iter().filter_map(|method| alias_of.get(method).cloned()).collect();

    let mut all = defined;
    all.extend(delegated);
    all.extend(via_alias);
    all
}

/// `(send nil? :delegate (sym $_)+ (hash <(pair (sym :to) _) ...>))`.
fn delegated_methods(call: &ruby_ast::node::CallNode<'_>) -> Vec<String> {
    let Some(arguments) = call.arguments() else { return Vec::new() };
    let list: Vec<Node<'_>> = arguments.arguments().iter().collect();
    let Some((hash, symbols)) = list.split_last() else { return Vec::new() };
    let elements = if let Some(hash) = hash.as_keyword_hash_node() {
        hash.elements()
    } else if let Some(hash) = hash.as_hash_node() {
        hash.elements()
    } else {
        return Vec::new();
    };
    let has_to = elements.iter().any(|element| {
        element
            .as_assoc_node()
            .is_some_and(|pair| pair.key().as_symbol_node().is_some_and(|k| k.unescaped() == b"to"))
    });
    let names: Vec<String> = symbols
        .iter()
        .filter_map(|s| {
            s.as_symbol_node().map(|s| String::from_utf8_lossy(s.unescaped()).into_owned())
        })
        .collect();
    if !has_to || symbols.is_empty() || names.len() != symbols.len() {
        return Vec::new();
    }
    names
}

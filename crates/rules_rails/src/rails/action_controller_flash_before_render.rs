//! `Rails/ActionControllerFlashBeforeRender`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/action_controller_flash_before_render.rb`.
//!
//! The upstream cop reasons about a whitequark tree (`right_siblings`,
//! `each_ancestor`); the helpers below recreate the parts of that shape the
//! cop depends on from the Prism tree, keeping the ancestor path explicitly.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::node::CallNode;
use ruby_ast::{each_descendant, for_each_child, Node, NodeExt as _};
use ruby_source::Span;

const MSG: &str = "Use `flash.now` before `render`.";

/// Use `flash.now` instead of `flash` before `render`.
#[derive(Debug, Clone)]
pub struct ActionControllerFlashBeforeRender;

impl Rule for ActionControllerFlashBeforeRender {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ActionControllerFlashBeforeRender",
        department: Department::Rails,
        summary: "Use `flash.now` instead of `flash` before `render`.",
        explanation: "Using `flash` assignment before `render` in Rails controllers will \
                      persist the message for too long. Check https://guides.rubyonrails.org/\
                      action_controller_overview.html#flash-now\n\nThis cop's autocorrection is \
                      unsafe because it changes how the message is persisted.\n\n```ruby\n# bad\n\
                      class HomeController < ApplicationController\n  def create\n    \
                      flash[:alert] = \"msg\"\n    render :index\n  end\nend\n\n# good\nclass \
                      HomeController < ApplicationController\n  def create\n    \
                      flash.now[:alert] = \"msg\"\n    render :index\n  end\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "A file consisting of a single top-level `flash[...] = ...` statement is \
                      never reported (RuboCop raises on it).",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let source = ctx.source().bytes();
        if !source.windows(5).any(|w| w == b"flash") {
            return;
        }
        let root = ctx.parsed().root();
        let mut hits = Vec::new();
        let mut path = Vec::new();
        walk(&root, &mut path, &mut hits);
        for span in hits {
            ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix {
                    applicability: Applicability::Unsafe,
                    edits: vec![Edit::replace(span, b"flash.now".to_vec())],
                },
            );
        }
    }
}

fn walk<'pr>(node: &Node<'pr>, path: &mut Vec<Node<'pr>>, hits: &mut Vec<Span>) {
    path.push(*node);
    if let Some(span) = flash_assignment_receiver(node) {
        if offense(path) {
            hits.push(span);
        }
    }
    for_each_child(node, |child| walk(child, path, hits));
    path.pop();
}

/// `^(send (send nil? :flash) :[]= ...)`: for the assignment `node`, the
/// span of its `flash` receiver.
fn flash_assignment_receiver(node: &Node<'_>) -> Option<Span> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"[]=" || call.is_safe_navigation() {
        return None;
    }
    let receiver = call.receiver()?;
    let flash = receiver.as_call_node()?;
    (flash.name().as_slice() == b"flash"
        && flash.receiver().is_none()
        && flash.arguments().is_none()
        && flash.block().is_none()
        && !flash.is_safe_navigation())
    .then(|| receiver.span())
}

/// `path` ends at the `[]=` node (`flash_assignment_node`).
fn offense(path: &[Node<'_>]) -> bool {
    let last = path.len() - 1;
    followed_by_render(path, last) == Some(true)
        && instance_method_or_block(path)
        && inherits_action_controller(path)
}

fn instance_method_or_block(path: &[Node<'_>]) -> bool {
    path.iter().any(|n| {
        n.as_def_node().is_some_and(|d| d.receiver().is_none())
            || n.as_lambda_node().is_some()
            || n.as_block_node().is_some_and(|b| {
                !b.parameters().is_some_and(|p| {
                    p.as_numbered_parameters_node().is_some() || p.as_it_parameters_node().is_some()
                })
            })
    })
}

fn inherits_action_controller(path: &[Node<'_>]) -> bool {
    let Some(class) = path.iter().rev().find(|n| n.as_class_node().is_some()) else {
        return false;
    };
    let mut found = false;
    each_descendant(class, &mut |node| {
        found = found || is_action_controller_const(node);
    });
    found
}

fn is_action_controller_const(node: &Node<'_>) -> bool {
    if is_bare_or_toplevel_const(node) {
        return const_name(node).as_deref() == Some("ApplicationController");
    }
    let Some(path) = node.as_constant_path_node() else { return false };
    path.name().is_some_and(|n| n.as_slice() == b"Base")
        && path.parent().is_some_and(|parent| {
            is_bare_or_toplevel_const(&parent)
                && const_name(&parent).as_deref() == Some("ActionController")
        })
}

fn is_if(node: &Node<'_>) -> bool {
    node.as_if_node().is_some() || node.as_unless_node().is_some()
}

/// Whether `path[k]` is a parser `rescue` node, given the node after it on
/// the path.
fn is_rescue_ancestor(path: &[Node<'_>], k: usize) -> bool {
    let node = &path[k];
    if node.as_rescue_modifier_node().is_some() {
        return true;
    }
    let Some(begin) = node.as_begin_node() else { return false };
    begin.rescue_clause().is_some()
        && path.get(k + 1).is_some_and(|next| next.as_ensure_node().is_none())
}

/// `followed_by_render?`; `None` where upstream would raise.
fn followed_by_render(path: &[Node<'_>], assign: usize) -> Option<bool> {
    let nearest = (0..assign).rev().find(|&k| is_if(&path[k]) || is_rescue_ancestor(path, k));
    let siblings;
    if let Some(k) = nearest {
        if use_redirect_to(&right_siblings(path, assign)) {
            return Some(false);
        }
        siblings = if path[k].as_begin_node().is_some() {
            rescue_siblings(&path[k])
        } else {
            right_siblings(path, k)
        };
    } else {
        let own = right_siblings(path, assign);
        if own.is_empty() {
            let parent_siblings = match parser_parent(path, assign)? {
                Parent::Real(k) => right_siblings(path, k),
                Parent::Virtual => Vec::new(),
            };
            if !use_redirect_to(&parent_siblings) {
                return Some(true);
            }
        }
        siblings = own;
    }
    Some(siblings.iter().any(contains_render))
}

fn contains_render(node: &Node<'_>) -> bool {
    let mut found = is_render(node);
    each_descendant(node, &mut |n| found = found || is_render(n));
    found
}

fn is_render(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|c| c.name().as_slice() == b"render" && c.receiver().is_none())
}

fn use_redirect_to(siblings: &[Node<'_>]) -> bool {
    siblings.iter().any(|sibling| {
        let mut node = *sibling;
        if let Some(ret) = sibling.as_return_node() {
            if let Some(arguments) = ret.arguments() {
                let mut it = arguments.arguments().iter();
                if let (Some(only), None) = (it.next(), it.next()) {
                    node = only;
                }
            }
        }
        node.as_call_node().is_some_and(|c: CallNode<'_>| {
            c.name().as_slice() == b"redirect_to"
                && !c.is_safe_navigation()
                && c.block().is_none_or(|b| b.as_block_node().is_none())
        })
    })
}

/// A parser node standing in for a Prism one: single-statement lists and
/// `else` wrappers collapse into their contents.
fn unwrap(node: Node<'_>) -> Option<Node<'_>> {
    if let Some(statements) = node.as_statements_node() {
        let mut body = statements.body().iter();
        return match (body.next(), body.next()) {
            (Some(only), None) => unwrap(only),
            (None, _) => None,
            _ => Some(node),
        };
    }
    if let Some(else_node) = node.as_else_node() {
        return else_node.statements().and_then(|s| unwrap(s.as_node()));
    }
    Some(node)
}

/// Right siblings of a rescue-bearing `begin`: its `ensure` body, if any.
fn rescue_siblings<'pr>(node: &Node<'pr>) -> Vec<Node<'pr>> {
    node.as_begin_node()
        .and_then(|b| b.ensure_clause())
        .and_then(|e| e.statements())
        .and_then(|s| unwrap(s.as_node()))
        .into_iter()
        .collect()
}

enum Parent {
    Real(usize),
    Virtual,
}

fn same(a: &Node<'_>, b: &Node<'_>) -> bool {
    a.kind() == b.kind() && a.span() == b.span()
}

fn statements_len(node: &Node<'_>) -> usize {
    node.as_statements_node().map_or(0, |s| s.body().iter().count())
}

/// The parser parent of `path[i]`: the nearest real ancestor, looking past
/// Prism-only wrappers; `None` at the root.
fn parser_parent(path: &[Node<'_>], i: usize) -> Option<Parent> {
    let mut j = i.checked_sub(1)?;
    loop {
        let node = &path[j];
        if node.as_statements_node().is_some() {
            if statements_len(node) >= 2 {
                return Some(Parent::Real(j));
            }
            if j == 0 || path[j - 1].as_program_node().is_some() {
                return None;
            }
            let up = &path[j - 1];
            if let Some(begin) = up.as_begin_node() {
                return Some(
                    if begin.rescue_clause().is_some() || begin.ensure_clause().is_some() {
                        Parent::Virtual
                    } else {
                        Parent::Real(j - 1)
                    },
                );
            }
            j -= 1;
        } else if node.as_else_node().is_some() || node.as_arguments_node().is_some() {
            j = j.checked_sub(1)?;
        } else if node.as_begin_node().is_some_and(|b| b.begin_keyword_loc().is_none()) {
            return Some(Parent::Virtual);
        } else if node.as_block_node().is_some() {
            return j.checked_sub(1).map(Parent::Real);
        } else {
            return Some(Parent::Real(j));
        }
    }
}

/// `node.right_siblings` as the parser tree sees them, for `path[i]`.
fn right_siblings<'pr>(path: &[Node<'pr>], i: usize) -> Vec<Node<'pr>> {
    let Some(p) = i.checked_sub(1) else { return Vec::new() };
    let node = &path[i];
    let parent = &path[p];
    let mut out: Vec<Option<Node<'pr>>> = Vec::new();
    if let Some(statements) = parent.as_statements_node() {
        let body: Vec<Node<'pr>> = statements.body().iter().collect();
        if body.len() >= 2 {
            let at = body.iter().position(|n| same(n, node));
            return at.map_or_else(Vec::new, |at| body[at + 1..].to_vec());
        }
        let boxed = p >= 1
            && (path[p - 1].as_parentheses_node().is_some()
                || path[p - 1]
                    .as_begin_node()
                    .is_some_and(|b| b.rescue_clause().is_none() && b.ensure_clause().is_none()));
        if boxed {
            return Vec::new();
        }
        if p >= 1 && path[p - 1].as_begin_node().is_some_and(|b| b.rescue_clause().is_none()) {
            return rescue_siblings(&path[p - 1]);
        }
        return right_siblings(path, p);
    } else if let Some(node_if) = parent.as_if_node() {
        if node_if.predicate().span() == node.span() {
            out.push(node_if.statements().and_then(|s| unwrap(s.as_node())));
            out.push(node_if.subsequent().and_then(unwrap));
        } else if node_if.statements().is_some_and(|s| same(&s.as_node(), node)) {
            out.push(node_if.subsequent().and_then(unwrap));
        }
    } else if let Some(unless) = parent.as_unless_node() {
        if unless.predicate().span() == node.span() {
            out.push(unless.else_clause().and_then(|e| unwrap(e.as_node())));
            out.push(unless.statements().and_then(|s| unwrap(s.as_node())));
        } else if unless.else_clause().is_some_and(|e| same(&e.as_node(), node)) {
            out.push(unless.statements().and_then(|s| unwrap(s.as_node())));
        }
    } else if parent.as_else_node().is_some() {
        return right_siblings(path, p);
    } else if let Some(arguments) = parent.as_arguments_node() {
        let list: Vec<Node<'pr>> = arguments.arguments().iter().collect();
        if let Some(at) = list.iter().position(|n| same(n, node)) {
            out.extend(list[at + 1..].iter().copied().map(Some));
        }
    } else if let Some(call) = parent.as_call_node() {
        if call.receiver().is_some_and(|r| same(&r, node)) {
            out.extend(call.arguments().map(|a| Some(a.as_node())));
            out.push(call.block());
        }
    }
    out.into_iter().flatten().collect()
}

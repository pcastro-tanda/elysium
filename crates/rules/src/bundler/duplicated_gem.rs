//! `Bundler/DuplicatedGem`, ported from RuboCop's
//! `lib/rubocop/cop/bundler/duplicated_gem.rb`.
//!
//! Upstream's `def_node_search '(send nil? :gem str ...)'` finds every bare
//! `gem` call whose first argument is a plain string literal, groups them by
//! that literal's value (`group_by(&:first_argument)`, structural equality
//! on the `str` node), and reports every occurrence after the first in each
//! group of two or more -- unless the whole group turns out to be one
//! declaration per mutually exclusive branch of a single `if`/`case`
//! (`conditional_declaration?`).
//!
//! Prism always wraps a branch body in a `StatementsNode` (never elides it
//! for a single statement the way whitequark's `begin` does), so "the
//! nearest ancestor that is not a `begin` node" becomes "the nearest
//! ancestor that is not a `StatementsNode`", and "is this node a direct
//! child of that branch" becomes "does this node's immediate parent
//! `StatementsNode` match one of the conditional's own branch bodies".
//! `elsif` is `IfNode` chained through `subsequent()`; `case`'s `when`
//! bodies are read off each `WhenNode` in `CaseNode::conditions()`, plus its
//! `else_clause()`.

use std::collections::HashMap;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{CaseNode, IfNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// One matched `gem 'name', ...` call.
#[derive(Debug, Clone)]
struct GemCall {
    /// The literal string argument's unescaped bytes -- the `group_by` key.
    key: Vec<u8>,
    /// The whole `send` node's span (the offense location).
    call_span: Span,
    /// The span of the immediate parent `StatementsNode`, when there is
    /// one, i.e. the branch body this call is a direct statement of.
    stmt_span: Option<Span>,
    /// The nearest ancestor that isn't a `StatementsNode`, when it is an
    /// `IfNode` or the `CaseNode` a `WhenNode` ancestor belongs to --
    /// upstream's `conditional_declaration?` `root_conditional_node`.
    root: Option<(NodeKind, Span)>,
}

/// Checks for duplicate gem entries in Gemfile.
#[derive(Debug, Clone, Default)]
pub struct DuplicatedGem {
    gem_calls: Vec<GemCall>,
    /// `IfNode` span -> its own `branches` (upstream's `IfNode#branches`,
    /// flattened through `elsif`), each a branch body `StatementsNode`'s
    /// span.
    if_branches: HashMap<Span, Vec<Span>>,
    /// `CaseNode` span -> each `when` body's and the `else` body's span
    /// (upstream's `CaseNode#branches`, with `nil` bodies already dropped,
    /// matching `.compact`).
    case_branches: HashMap<Span, Vec<Span>>,
}

/// RuboCop's `IfNode#branches`, flattened recursively through `elsif`
/// (Prism's `subsequent()` chain of nested `IfNode`s), ending at a plain
/// `else` (`ElseNode`) or nothing.
fn if_branch_spans(node: &IfNode<'_>) -> Vec<Span> {
    let mut spans = Vec::new();
    if let Some(statements) = node.statements() {
        spans.push(statements.location().span());
    }
    if let Some(subsequent) = node.subsequent() {
        if let Some(elsif) = subsequent.as_if_node() {
            spans.extend(if_branch_spans(&elsif));
        } else if let Some(else_node) = subsequent.as_else_node() {
            if let Some(statements) = else_node.statements() {
                spans.push(statements.location().span());
            }
        }
    }
    spans
}

/// RuboCop's `CaseNode#branches` (`.compact`ed): each `when` body's span,
/// then the `else` body's span if any.
fn case_branch_spans(node: &CaseNode<'_>) -> Vec<Span> {
    let mut spans = Vec::new();
    for condition in &node.conditions() {
        if let Some(when) = condition.as_when_node() {
            if let Some(statements) = when.statements() {
                spans.push(statements.location().span());
            }
        }
    }
    if let Some(else_clause) = node.else_clause() {
        if let Some(statements) = else_clause.statements() {
            spans.push(statements.location().span());
        }
    }
    spans
}

/// Matches upstream's `(send nil? :gem str ...)` node pattern against one
/// `CallNode`, plus the ancestor bookkeeping `conditional_declaration?`
/// needs later.
fn gem_call(node: &Node<'_>, ctx: &Context<'_>) -> Option<GemCall> {
    let call = node.as_call_node()?;
    if call.receiver().is_some() {
        return None;
    }
    if call.name().as_slice() != b"gem" {
        return None;
    }
    let first_arg = call.arguments()?.arguments().first()?;
    let string = first_arg.as_string_node()?;

    let ancestors = ctx.ancestors();
    let stmt_span = ancestors
        .last()
        .filter(|ancestor| ancestor.kind == NodeKind::StatementsNode)
        .map(|ancestor| ancestor.span);

    // Upstream's `each_ancestor.find { |a| !a.begin_type? }`: the nearest
    // ancestor, scanning from the immediate parent outward, that isn't a
    // `StatementsNode` (Prism's `begin`).
    let mut root = None;
    for (i, ancestor) in ancestors.iter().enumerate().rev() {
        if ancestor.kind == NodeKind::StatementsNode {
            continue;
        }
        if ancestor.kind == NodeKind::IfNode {
            root = Some((NodeKind::IfNode, ancestor.span));
        } else if ancestor.kind == NodeKind::WhenNode && i > 0 {
            // `parent.parent` when `parent` is a `when`: the enclosing
            // `case`, the ancestor just above it.
            root = Some((NodeKind::CaseNode, ancestors[i - 1].span));
        }
        break;
    }

    Some(GemCall { key: string.unescaped().to_vec(), call_span: node.span(), stmt_span, root })
}

impl DuplicatedGem {
    /// Upstream's `conditional_declaration?`: true when every node in the
    /// group is a direct statement of a distinct branch of the same single
    /// `if`/`case` that `nodes.first` sits directly in.
    ///
    /// `within_conditional?`'s `branch.child_nodes.include?(node)` is an
    /// `Array#include?` scan using whitequark `Parser::AST::Node#==`:
    /// structural equality of the whole call node (method name + every
    /// argument, recursively), ignoring source location entirely -- not
    /// "is this literally that ancestry slot". So a branch matches not only
    /// the call that actually sits there, but any *other* direct statement
    /// of that branch that happens to be a structurally identical `gem`
    /// call (e.g. two bare `gem "redcarpet"` with no extra args, one inside
    /// the `if`, one inside an unrelated `group` block elsewhere: upstream
    /// still treats the second as "in" the first's branch and skips it).
    /// This port approximates that structural equality with exact
    /// source-text comparison of the whole call, matching this codebase's
    /// established approximation for the same problem elsewhere (see
    /// `Lint/DuplicateElsifCondition`, `Lint/DuplicateCaseCondition`).
    fn conditional_declaration(&self, indices: &[usize], ctx: &Context<'_>) -> bool {
        let Some(first) = indices.first().map(|&i| &self.gem_calls[i]) else { return false };
        let Some((kind, span)) = first.root else { return false };
        let Some(branches) = (match kind {
            NodeKind::IfNode => self.if_branches.get(&span),
            _ => self.case_branches.get(&span),
        }) else {
            return false;
        };
        indices.iter().all(|&i| {
            let text = ctx.text(self.gem_calls[i].call_span);
            self.gem_calls.iter().any(|other| {
                other.stmt_span.is_some_and(|s| branches.contains(&s))
                    && ctx.text(other.call_span) == text
            })
        })
    }
}

impl Rule for DuplicatedGem {
    const META: RuleMeta = RuleMeta {
        name: "Bundler/DuplicatedGem",
        department: Department::Bundler,
        summary: "Checks for duplicate gem entries in Gemfile.",
        explanation: "\
A Gem's requirements should be listed only once in a Gemfile.

```ruby
# bad
gem 'rubocop'
gem 'rubocop'

# bad
group :development do
  gem 'rubocop'
end

group :test do
  gem 'rubocop'
end

# good
group :development, :test do
  gem 'rubocop'
end

# good
gem 'rubocop', groups: [:development, :test]

# good - conditional declaration
if Dir.exist?(local)
  gem 'rubocop', path: local
elsif ENV['RUBOCOP_VERSION'] == 'master'
  gem 'rubocop', git: 'https://github.com/rubocop/rubocop.git'
else
  gem 'rubocop', '~> 0.90.0'
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::IfNode, NodeKind::CaseNode],
        config: &[],
        blind_spots: "\
Only recognizes a plain string literal as the gem name, matching upstream's
`str` node pattern: `gem name_variable` or `gem \"#{prefix}-rubocop\"` never
groups with anything, even another identical interpolation.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !file_is_gemfile_like(ctx) {
            return;
        }
        match node.kind() {
            NodeKind::IfNode => {
                if let Some(if_node) = node.as_if_node() {
                    self.if_branches.insert(node.span(), if_branch_spans(&if_node));
                }
            }
            NodeKind::CaseNode => {
                if let Some(case_node) = node.as_case_node() {
                    self.case_branches.insert(node.span(), case_branch_spans(&case_node));
                }
            }
            NodeKind::CallNode => {
                if let Some(call) = gem_call(node, ctx) {
                    self.gem_calls.push(call);
                }
            }
            _ => {}
        }
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let mut groups: Vec<(Vec<u8>, Vec<usize>)> = Vec::new();
        for (i, call) in self.gem_calls.iter().enumerate() {
            match groups.iter_mut().find(|(key, _)| *key == call.key) {
                Some((_, indices)) => indices.push(i),
                None => groups.push((call.key.clone(), vec![i])),
            }
        }

        for (name, indices) in &groups {
            if indices.len() < 2 || self.conditional_declaration(indices, ctx) {
                continue;
            }
            let first_line = ctx.line_col(self.gem_calls[indices[0]].call_span.start).line;
            let gem_name = String::from_utf8_lossy(name);
            for &i in &indices[1..] {
                let call_span = self.gem_calls[i].call_span;
                ctx.report(
                    &Self::META,
                    call_span,
                    format!(
                        "Gem `{gem_name}` requirements already given on line {first_line} of \
                         the Gemfile."
                    ),
                );
            }
        }
    }
}

/// Upstream's own `Include` (`config/default.yml`): `**/*.gemfile`,
/// `**/Gemfile`, `**/gems.rb`. The fixture harness runs every cop against
/// every case regardless of `Include`/`Exclude` (those apply only to the
/// real CLI's file discovery, `LoadedConfig::is_cop_targeting`), so a
/// file-name-dependent cop like this one has to re-check its own `Include`
/// against the case's `# file:` name itself.
fn file_is_gemfile_like(ctx: &Context<'_>) -> bool {
    let path = ctx.source().path();
    let Some(basename) = path.file_name().and_then(|name| name.to_str()) else { return false };
    basename == "Gemfile" || basename == "gems.rb" || basename.ends_with(".gemfile")
}

//! `Gemspec/OrderedDependencies`, ported from RuboCop's
//! `lib/rubocop/cop/gemspec/ordered_dependencies.rb` plus the
//! `OrderedGemNode` mixin and `OrderedGemCorrector` it uses (also shared by
//! `Bundler/OrderedGems`).
//!
//! # Matched shape
//!
//! Upstream's `def_node_search` pattern is `(send (lvar _) {:add_dependency
//! :add_runtime_dependency :add_development_dependency} (str _) ...)`,
//! searched over the *whole file* (not just inside a `Gem::Specification.new`
//! block): a receiverful call whose receiver is a plain local variable read
//! (so `self.add_dependency ...` or a chained receiver like
//! `x.spec.add_dependency ...` never match), one of the three dependency
//! method names, and whose first argument is a bare string literal --
//! `spec.add_dependency 'rubocop'.freeze` or `spec.add_dependency dep` (a
//! local variable) never match either, since the first argument node itself
//! is not a `(str _)`. [`dependency_declaration`] reproduces this shape
//! directly on each [`ruby_ast::node::CallNode`] the walk visits (in document
//! order, matching `def_node_search`'s preorder traversal), guarding
//! `!call.is_safe_navigation()` since a `send`-only pattern excludes `&.`.
//!
//! # Consecutiveness and comment association
//!
//! `consecutive_lines?` requires the previous declaration's own last line to
//! be exactly one less than the *current* declaration's "source range" first
//! line, where that source range is the declaration's own node unless
//! `TreatCommentsAsGroupSeparators` is false, in which case it is extended
//! back to the topmost line of a contiguous run of standalone comment lines
//! directly above it with no blank-line gap (`ast_with_comments[node].first`,
//! the `parser` gem's comment/node association). [`leading_start`] walks
//! upward from a declaration's own line while the immediately preceding line
//! is entirely a comment (nothing but whitespace before the `#`), matching
//! that association without needing a real token stream. This means a
//! `TreatCommentsAsGroupSeparators: true` comment block breaks
//! consecutiveness (the lines in between are skipped over, seen as a group
//! separator), while `false` folds the comment into the current
//! declaration's block, so a bare blank line remains the only thing that
//! breaks it.
//!
//! # Autocorrection
//!
//! `OrderedGemCorrector#correct` builds `declaration_with_comment` ranges for
//! both nodes -- from the topmost comment-or-own line through the *node's
//! own* last line inclusive of its trailing newline (regardless of
//! `TreatCommentsAsGroupSeparators`, this end never includes a *trailing*
//! comment on a later line) -- and swaps them outright via
//! `Corrector#swap`. [`declaration_range`] computes the same span with
//! [`linter::Context::whole_lines`], which already implements
//! `range_by_whole_lines(..., include_final_newline: true)`; the swap itself
//! is two [`Edit::replace`]s exchanging each range's text.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`, `%<previous>s`/`%<current>s` filled in with
/// `register_offense`'s `gem_name(current)`/`gem_name(previous)`
/// respectively (yes, the flagged, later-appearing-in-file node's own name
/// is "previous" in the message -- it is the one that should come first
/// alphabetically).
fn message(current_name: &[u8], previous_name: &[u8]) -> String {
    format!(
        "Dependencies should be sorted in an alphabetical order within their section of the \
         gemspec. Dependency `{}` should appear before `{}`.",
        String::from_utf8_lossy(current_name),
        String::from_utf8_lossy(previous_name)
    )
}

/// The three dependency-declaring method names this cop compares within,
/// each group independently (`get_dependency_name(previous) ==
/// get_dependency_name(current)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DependencyMethod {
    Add,
    Runtime,
    Development,
}

fn dependency_method(name: &[u8]) -> Option<DependencyMethod> {
    match name {
        b"add_dependency" => Some(DependencyMethod::Add),
        b"add_runtime_dependency" => Some(DependencyMethod::Runtime),
        b"add_development_dependency" => Some(DependencyMethod::Development),
        _ => None,
    }
}

/// One matched dependency declaration: `dependency_declarations`'s node,
/// reduced to what the rest of the cop needs.
#[derive(Debug, Clone)]
struct Declaration {
    /// The `send` node's own span (excludes any leading comment).
    span: Span,
    method: DependencyMethod,
    /// `gem_name`: the first argument string literal's unescaped content.
    name: Vec<u8>,
}

/// RuboCop's `def_node_search` pattern applied to one `CallNode`: see the
/// module doc's "Matched shape" section.
fn dependency_declaration(node: &Node<'_>) -> Option<Declaration> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() {
        return None;
    }
    let receiver = call.receiver()?;
    receiver.as_local_variable_read_node()?;
    let method = dependency_method(call.name().as_slice())?;
    let first_arg = call.arguments()?.arguments().first()?;
    let string = first_arg.as_string_node()?;
    Some(Declaration { span: call.as_node().span(), method, name: string.unescaped().to_vec() })
}

/// `OrderedGemNode#gem_canonical_name`: strips `-`/`_` unless
/// `ConsiderPunctuation`, then lowercases -- compared byte-wise here, which
/// agrees with Ruby's `String#<=>` for the ASCII gem names this applies to.
fn canonical_name(name: &[u8], consider_punctuation: bool) -> Vec<u8> {
    name.iter()
        .copied()
        .filter(|&b| consider_punctuation || (b != b'-' && b != b'_'))
        .map(|b| b.to_ascii_lowercase())
        .collect()
}

/// Whether `line` (1-based) consists of nothing but a comment -- only
/// whitespace precedes the `#` -- so it counts as a line the `parser` gem's
/// comment/node associator could claim for the node just below it.
fn standalone_comment_start(ctx: &Context<'_>, line: u32) -> Option<u32> {
    let comment = ctx.comments().iter().find(|c| c.line == line)?;
    let line_span = ctx.line_span(line);
    let before = ctx.text(Span::new(line_span.start, comment.span.start));
    before.iter().all(u8::is_ascii_whitespace).then_some(comment.span.start)
}

/// The topmost line's start offset of the contiguous run of standalone
/// comment lines directly above `span`'s own start line -- upstream's
/// `ast_with_comments[node].first.source_range`, see the module doc's
/// "Consecutiveness and comment association" section. Returns `span.start`
/// itself when there is no such comment directly above.
fn leading_start(ctx: &Context<'_>, span: Span) -> u32 {
    let mut line = ctx.line_col(span.start).line;
    let mut start = span.start;
    while line > 1 {
        let Some(comment_start) = standalone_comment_start(ctx, line - 1) else { break };
        start = comment_start;
        line -= 1;
    }
    start
}

/// `OrderedGemNode#get_source_range(node, comments_as_separators).first_line`.
fn declaration_first_line(
    ctx: &Context<'_>,
    span: Span,
    treat_comments_as_separators: bool,
) -> u32 {
    if treat_comments_as_separators {
        ctx.line_col(span.start).line
    } else {
        ctx.line_col(leading_start(ctx, span)).line
    }
}

/// `OrderedGemNode#consecutive_lines?`.
fn consecutive(
    ctx: &Context<'_>,
    previous: Span,
    current: Span,
    treat_comments_as_separators: bool,
) -> bool {
    let first_line = declaration_first_line(ctx, current, treat_comments_as_separators);
    ctx.last_line(previous) + 1 == first_line
}

/// `OrderedGemCorrector#declaration_with_comment`: see the module doc's
/// "Autocorrection" section.
fn declaration_range(ctx: &Context<'_>, span: Span, treat_comments_as_separators: bool) -> Span {
    let start = if treat_comments_as_separators { span.start } else { leading_start(ctx, span) };
    ctx.whole_lines(Span::new(start, span.end))
}

/// Dependencies in the gemspec should be alphabetically sorted.
///
/// ```ruby
/// # bad
/// spec.add_dependency 'rubocop'
/// spec.add_dependency 'rspec'
///
/// # good
/// spec.add_dependency 'rspec'
/// spec.add_dependency 'rubocop'
///
/// # good
/// spec.add_dependency 'rubocop'
///
/// spec.add_dependency 'rspec'
/// ```
#[derive(Debug, Clone, Default)]
pub struct OrderedDependencies {
    treat_comments_as_separators: bool,
    consider_punctuation: bool,
    /// Every dependency declaration seen so far this file, in document
    /// order -- upstream's `dependency_declarations(processed_source.ast)`.
    declarations: Vec<Declaration>,
}

impl Rule for OrderedDependencies {
    const META: RuleMeta = RuleMeta {
        name: "Gemspec/OrderedDependencies",
        department: Department::Gemspec,
        summary: "Dependencies in the gemspec should be alphabetically sorted.",
        explanation: "\
Dependencies in the gemspec should be alphabetically sorted.

```ruby
# bad
spec.add_dependency 'rubocop'
spec.add_dependency 'rspec'

# good
spec.add_dependency 'rspec'
spec.add_dependency 'rubocop'

# good
spec.add_dependency 'rubocop'

spec.add_dependency 'rspec'
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "TreatCommentsAsGroupSeparators",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "A comment directly above a dependency breaks up its group, so gems on \
                      either side of it are compared separately.",
            },
            ConfigOption {
                name: "ConsiderPunctuation",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "By default, `-` and `_` are ignored for order purposes; set this to \
                      compare them too.",
            },
        ],
        blind_spots: "\
`Include: ['**/*.gemspec']` restricts this cop to gemspec files upstream; elysium applies that
restriction at the config-file-matching layer (see `crates/config`), not in this rule's own logic.
Matching the upstream `def_node_search`, declarations are searched for anywhere in the file, not
only inside a `Gem::Specification.new` block.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            treat_comments_as_separators: options.bool("TreatCommentsAsGroupSeparators"),
            consider_punctuation: options.bool("ConsiderPunctuation"),
            declarations: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.declarations.clear();
    }

    fn enter(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if let Some(declaration) = dependency_declaration(node) {
            self.declarations.push(declaration);
        }
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let declarations = std::mem::take(&mut self.declarations);
        for window in declarations.windows(2) {
            let previous = &window[0];
            let current = &window[1];
            if !consecutive(ctx, previous.span, current.span, self.treat_comments_as_separators) {
                continue;
            }
            let current_canon = canonical_name(&current.name, self.consider_punctuation);
            let previous_canon = canonical_name(&previous.name, self.consider_punctuation);
            if current_canon >= previous_canon {
                continue;
            }
            if previous.method != current.method {
                continue;
            }

            let previous_range =
                declaration_range(ctx, previous.span, self.treat_comments_as_separators);
            let current_range =
                declaration_range(ctx, current.span, self.treat_comments_as_separators);
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(current_range, ctx.text(previous_range).to_vec()),
                    Edit::replace(previous_range, ctx.text(current_range).to_vec()),
                ],
            };
            ctx.report_with_fix(
                &Self::META,
                current.span,
                message(&current.name, &previous.name),
                fix,
            );
        }
    }
}

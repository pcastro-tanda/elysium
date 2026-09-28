//! `Bundler/OrderedGems`, ported from RuboCop's
//! `lib/rubocop/cop/bundler/ordered_gems.rb` plus the `OrderedGemNode` mixin
//! and `OrderedGemCorrector` it uses (also shared by
//! `Gemspec/OrderedDependencies`, see `gemspec/ordered_dependencies.rs` for
//! an identical port of the shared mixin logic -- this file inlines its own
//! private copy per the porting kit's "copy shared helpers privately" rule).
//!
//! # Matched shape
//!
//! Upstream's `def_node_search` pattern is `(:send nil? :gem str ...)`: a
//! receiverless call named `gem` whose first argument is a bare string
//! literal -- `gem NAME_CONST` or `gem name.freeze` never match, since the
//! first argument node itself is not a `(str _)`. [`gem_declaration`]
//! reproduces this shape on each [`Node`] the walk visits (in document
//! order, matching `def_node_search`'s preorder traversal, including gems
//! nested inside a `group do ... end` block).
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
        "Gems should be sorted in an alphabetical order within their section of the Gemfile. \
         Gem `{}` should appear before `{}`.",
        String::from_utf8_lossy(current_name),
        String::from_utf8_lossy(previous_name)
    )
}

/// One matched gem declaration: `gem_declarations`'s node, reduced to what
/// the rest of the cop needs.
#[derive(Debug, Clone)]
struct Declaration {
    /// The `send` node's own span (excludes any leading comment).
    span: Span,
    /// `gem_name`: the first argument string literal's unescaped content.
    name: Vec<u8>,
}

/// RuboCop's `def_node_search` pattern applied to one `CallNode`: see the
/// module doc's "Matched shape" section.
fn gem_declaration(node: &Node<'_>) -> Option<Declaration> {
    let call = node.as_call_node()?;
    if call.receiver().is_some() {
        return None;
    }
    if call.name().as_slice() != b"gem" {
        return None;
    }
    let first_arg = call.arguments()?.arguments().first()?;
    let string = first_arg.as_string_node()?;
    Some(Declaration { span: call.as_node().span(), name: string.unescaped().to_vec() })
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

/// Gems within groups in the Gemfile should be alphabetically sorted.
///
/// ```ruby
/// # bad
/// gem 'rubocop'
/// gem 'rspec'
///
/// # good
/// gem 'rspec'
/// gem 'rubocop'
///
/// # good
/// gem 'rubocop'
///
/// gem 'rspec'
/// ```
#[derive(Debug, Clone, Default)]
pub struct OrderedGems {
    treat_comments_as_separators: bool,
    consider_punctuation: bool,
    /// Every gem declaration seen so far this file, in document order --
    /// upstream's `gem_declarations(processed_source.ast)`.
    declarations: Vec<Declaration>,
}

impl Rule for OrderedGems {
    const META: RuleMeta = RuleMeta {
        name: "Bundler/OrderedGems",
        department: Department::Bundler,
        summary: "Gems within groups in the Gemfile should be alphabetically sorted.",
        explanation: "\
Gems should be alphabetically sorted within groups.

```ruby
# bad
gem 'rubocop'
gem 'rspec'

# good
gem 'rspec'
gem 'rubocop'

# good
gem 'rubocop'

gem 'rspec'
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
                doc: "A comment directly above a gem breaks up its group, so gems on either \
                      side of it are compared separately.",
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
`Include: ['**/*.gemfile', '**/Gemfile', '**/gems.rb']` restricts this cop to Gemfile-like files
upstream; elysium applies that restriction at the config-file-matching layer (see
`crates/config`), not in this rule's own logic.",
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
        if let Some(declaration) = gem_declaration(node) {
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

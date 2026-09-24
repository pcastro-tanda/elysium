//! `Layout/FirstHashElementIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/first_hash_element_indentation.rb` plus the
//! `Alignment`, `ConfigurableEnforcedStyle`, and `MultilineElementIndentation`
//! mixins it includes.
//!
//! RuboCop resolves two independent things for each hash literal:
//!
//! - **`left_parenthesis`** (RuboCop's `on_send`/`each_argument_node`/
//!   `on_node(:hash, arg, :send)`): when a hash literal argument's own `{`
//!   shares its source line with the enclosing call's `(`, indentation is
//!   measured from that call's opening parenthesis instead of from the
//!   start of the hash's own line. Because the engine here is a single
//!   top-down pass, this has to be resolved *before* the pass naturally
//!   reaches the hash (its governing call is an ancestor), so
//!   [`FirstHashElementIndentation::enter`]'s `CallNode` arm eagerly walks
//!   the call's own argument subtree (bounded to that one call, mirroring
//!   RuboCop's own bounded recursive `on_node` search) and, for every
//!   qualifying hash, both marks it `ignored` (so the natural `HashNode`
//!   visit below does not double-check it) and calls `check` immediately.
//! - **the `:parent_hash_key` basis** (RuboCop's
//!   `hash_pair_where_value_beginning_with`/
//!   `right_sibling_begins_on_subsequent_line?`): when a hash literal is
//!   itself the value of a `key: { ... }` pair whose key starts on the same
//!   line as the hash's own `{`, and that pair has a sibling pair starting
//!   on a later line, indentation is measured from the key's own column
//!   instead. This is purely a tree-shape fact about the hash's immediate
//!   parent and its siblings, unrelated to calls, so it is recorded once
//!   per hash/keyword-hash literal's own direct pairs as the engine
//!   naturally visits each one (`record_facts_one_level`, keyed by each
//!   pair's *value* span in `parent_facts`) and looked up by the value hash
//!   when the traversal reaches it. The eager call-argument walk above
//!   duplicates this same one-level computation locally (`eager_check_pairs`)
//!   because it runs before the engine's own visit of the containing
//!   hash/keyword-hash has populated `parent_facts` for it.

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{AssocNode, HashNode};
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, NodeKind, NodeList, Visitor};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    SpecialInsideParentheses,
    Consistent,
    AlignBraces,
}

/// RuboCop's `indent_base_type` return values, driving both offense
/// messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaseType {
    LeftBraceOrBracket,
    FirstColumnAfterLeftParenthesis,
    ParentHashKey,
    StartOfLine,
}

impl BaseType {
    /// RuboCop's `base_description`.
    fn description(self) -> &'static str {
        match self {
            BaseType::LeftBraceOrBracket => "the position of the opening brace",
            BaseType::FirstColumnAfterLeftParenthesis => {
                "the first position after the preceding left parenthesis"
            }
            BaseType::ParentHashKey => "the parent hash key",
            BaseType::StartOfLine => "the start of the line where the left curly brace is",
        }
    }

    /// RuboCop's `message_for_right_brace`.
    fn right_brace_message(self) -> &'static str {
        match self {
            BaseType::LeftBraceOrBracket => "Indent the right brace the same as the left brace.",
            BaseType::FirstColumnAfterLeftParenthesis => {
                "Indent the right brace the same as the first position after the preceding left \
                 parenthesis."
            }
            BaseType::ParentHashKey => "Indent the right brace the same as the parent hash key.",
            BaseType::StartOfLine => {
                "Indent the right brace the same as the start of the line where the left brace is."
            }
        }
    }
}

/// RuboCop's `MSG`.
fn message(width: i64, base_description: &str) -> String {
    format!("Use {width} spaces for indentation in a hash, relative to {base_description}.")
}

/// RuboCop's `hash_pair_where_value_beginning_with`/
/// `right_sibling_begins_on_subsequent_line?`, precomputed for a hash
/// literal that is the value of some pair, keyed by that hash's own span.
#[derive(Debug, Clone, Copy)]
struct ParentPairFact {
    /// The enclosing pair's own column (RuboCop's `pair.loc.column`).
    column: u32,
    /// The line the enclosing pair's key starts on.
    key_line: u32,
    /// Whether the enclosing pair has a sibling (in its own hash/keyword-hash
    /// literal) that starts on a line after the pair's own last line.
    right_sibling_later: bool,
}

/// Checks the indentation of the first key in a hash literal, ported from
/// RuboCop's `FirstHashElementIndentation` cop plus its `Alignment`,
/// `ConfigurableEnforcedStyle`, and `MultilineElementIndentation` mixins.
#[derive(Debug, Clone)]
pub struct FirstHashElementIndentation {
    style: Style,
    indentation_width: i64,
    /// `Layout/HashAlignment`'s `EnforcedColonStyle == 'separator'`.
    colon_separator: bool,
    /// `Layout/HashAlignment`'s `EnforcedHashRocketStyle == 'separator'`.
    hash_rocket_separator: bool,
    /// `Layout/ArgumentAlignment`'s `EnforcedStyle == 'with_fixed_indentation'`.
    skip_send: bool,
    /// Hash literals already `check`ed eagerly from a governing call's
    /// argument list (RuboCop's `ignored_node?`).
    ignored: HashSet<Span>,
    /// `ParentPairFact`s recorded for hash literals reachable as a plain
    /// pair value, keyed by the hash's own span.
    parent_facts: HashMap<Span, ParentPairFact>,
}

impl Rule for FirstHashElementIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/FirstHashElementIndentation",
        department: Department::Layout,
        summary: "Checks the indentation of the first key in a hash literal.",
        explanation: "\
Checks the indentation of the first key in a hash literal where the opening
brace and the first key are on separate lines. The other keys' indentations
are handled by `Layout/HashAlignment`.

By default, `Hash` literals that are arguments in a method call with
parentheses, and where the opening curly brace of the hash is on the same
line as the opening parenthesis of the method call, shall have their first
key indented one step (two spaces) more than the position inside the opening
parenthesis. Other hash literals shall have their first key indented one
step more than the start of the line where the opening curly brace is. This
default style is called `special_inside_parentheses`.

```ruby
# EnforcedStyle: special_inside_parentheses (default)

# bad
hash = {
  key: :value
}
and_in_a_method_call({
  no: :difference
                     })

# good
special_inside_parentheses
hash = {
  key: :value
}
but_in_a_method_call({
                        its_like: :this
                      })
```

```ruby
# EnforcedStyle: consistent

# bad
hash = {
  key: :value
}
but_in_a_method_call({
                        its_like: :this
                       })

# good
hash = {
  key: :value
}
and_in_a_method_call({
  no: :difference
})
```

```ruby
# EnforcedStyle: align_braces

# bad
and_now_for_something = {
                          completely: :different
}

# good
and_now_for_something = {
                          completely: :different
                        }
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::HashNode, NodeKind::KeywordHashNode, NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("special_inside_parentheses"),
                allowed: &["special_inside_parentheses", "consistent", "align_braces"],
                doc: "Whether a hash literal argument's first key is indented relative to the \
                      preceding left parenthesis when the brace shares its line \
                      (`special_inside_parentheses`), always relative to the start of the \
                      hash's own line (`consistent`), or relative to the opening brace's own \
                      column (`align_braces`).",
            },
            ConfigOption {
                name: "IndentationWidth",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "Number of spaces for the first key's indentation, overriding \
                      `Layout/IndentationWidth`'s `Width` (which itself defaults to 2).",
            },
        ],
        blind_spots: "\
RuboCop's `each_argument_node` resolves the governing call for a hash \
argument via a fully generic recursive `on_node` search that descends \
through *any* wrapper node type (arrays, ternaries, boolean connectives, \
splats, method chains, ...) stopping only at a nested `send`/`csend`. This \
port narrows that search to hash-literal/keyword-hash-literal pair chains \
only (`eager_check_call_hash`/`eager_check_pairs`): a hash argument buried \
inside e.g. an array literal or a ternary is treated as an ordinary \
top-level hash literal (checked against the start of its own line) rather \
than against the call's parenthesis, a false-negative-only divergence for \
the `special_inside_parentheses`/`consistent` distinction in that shape.

RuboCop's `MultilineElementIndentation#right_sibling` is the pair's true \
next AST sibling regardless of type; this port's sibling lookahead \
(`record_facts_one_level`/`eager_check_pairs`) matches that (it looks at \
the next raw hash/keyword-hash element, not the next *pair*, so a \
`**splat` between two pairs is not skipped over).

The `ambiguous_style_detected`/`correct_style_detected`/`detected_styles` \
bookkeeping RuboCop's `MultilineElementIndentation` mixin performs (used \
only for `--auto-gen-config` style inference) is not replicated, since it \
never itself produces an offense in a single lint run.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "consistent" => Style::Consistent,
            "align_braces" => Style::AlignBraces,
            _ => Style::SpecialInsideParentheses,
        };
        let indentation_width = options
            .get("IndentationWidth")
            .and_then(OptionValue::as_int)
            .or_else(|| {
                options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
            })
            .unwrap_or(2);
        let colon_separator = options
            .peer("Layout/HashAlignment", "EnforcedColonStyle")
            .and_then(OptionValue::as_str)
            == Some("separator");
        let hash_rocket_separator = options
            .peer("Layout/HashAlignment", "EnforcedHashRocketStyle")
            .and_then(OptionValue::as_str)
            == Some("separator");
        let skip_send =
            options.peer("Layout/ArgumentAlignment", "EnforcedStyle").and_then(OptionValue::as_str)
                == Some("with_fixed_indentation");
        Ok(Self {
            style,
            indentation_width,
            colon_separator,
            hash_rocket_separator,
            skip_send,
            ignored: HashSet::new(),
            parent_facts: HashMap::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.ignored.clear();
        self.parent_facts.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::HashNode => {
                let hash = node.as_hash_node().expect("kind matched");
                let span = hash.location().span();
                if !self.ignored.contains(&span) {
                    let fact = self.parent_facts.get(&span).copied();
                    self.check(ctx, &hash, None, fact);
                }
                self.record_facts_one_level(ctx, &hash.elements());
            }
            NodeKind::KeywordHashNode => {
                let kw = node.as_keyword_hash_node().expect("kind matched");
                self.record_facts_one_level(ctx, &kw.elements());
            }
            NodeKind::CallNode => {
                if self.skip_send {
                    return;
                }
                let call = node.as_call_node().expect("kind matched");
                // RuboCop's `node.loc.begin`: for a `parser`-gem `send` node
                // this is only set for an actual `(...)` argument list --
                // `foo[bar]`/`foo[bar] = baz` (Prism: a `[]`/`[]=` call)
                // carry their brackets in the same `opening_loc` slot, but
                // upstream's `Send` location map leaves `begin`/`end` nil
                // for those, so `each_argument_node` never treats the `[`
                // as a governing left parenthesis.
                let (Some(open_loc), Some(args)) = (call.opening_loc(), call.arguments()) else {
                    return;
                };
                let open_span = open_loc.span();
                if ctx.text(open_span).first() != Some(&b'(') {
                    return;
                }
                let open_line = ctx.line_col(open_span.start).line;
                for arg in &args.arguments() {
                    self.eager_check_call_hash(ctx, &arg, open_span, open_line, None);
                }
            }
            _ => {}
        }
    }
}

impl FirstHashElementIndentation {
    /// RuboCop's `each_argument_node`/`on_node(:hash, arg, :send)`: finds
    /// every explicit-brace hash literal reachable from `node` without
    /// crossing into a nested call's own arguments (see `META.blind_spots`
    /// for how this narrows RuboCop's fully generic recursive search), and
    /// -- for each one whose own opening brace shares `open_line` with the
    /// enclosing call's opening parenthesis -- `check`s it eagerly with
    /// `parent_fact` as its precomputed `:parent_hash_key` basis, marking it
    /// `ignored` so the natural `HashNode` visit does not double-check it.
    fn eager_check_call_hash(
        &mut self,
        ctx: &mut Context<'_>,
        node: &Node<'_>,
        open_span: Span,
        open_line: u32,
        parent_fact: Option<ParentPairFact>,
    ) {
        if let Some(hash) = node.as_hash_node() {
            let brace_span = hash.opening_loc().span();
            if ctx.line_col(brace_span.start).line == open_line {
                self.ignored.insert(hash.location().span());
                self.check(ctx, &hash, Some(open_span), parent_fact);
            }
            self.eager_check_pairs(ctx, &hash.elements(), open_span, open_line);
            return;
        }
        if let Some(kw) = node.as_keyword_hash_node() {
            self.eager_check_pairs(ctx, &kw.elements(), open_span, open_line);
        }
    }

    /// One level of `eager_check_call_hash`'s pair walk: computes each
    /// pair's own `ParentPairFact` locally (duplicating
    /// `record_facts_one_level`'s formula -- see the module doc comment for
    /// why) and recurses into its value.
    fn eager_check_pairs(
        &mut self,
        ctx: &mut Context<'_>,
        elements: &NodeList<'_>,
        open_span: Span,
        open_line: u32,
    ) {
        let items: Vec<Node<'_>> = elements.iter().collect();
        for (i, item) in items.iter().enumerate() {
            let Some(pair) = item.as_assoc_node() else { continue };
            let pair_span = pair.location().span();
            let key_line = ctx.line_col(pair_span.start).line;
            let right_sibling_later = items.get(i + 1).is_some_and(|sib| {
                let last_line = last_line_of(ctx, pair_span);
                ctx.line_col(sib.span().start).line > last_line
            });
            let fact = ParentPairFact {
                column: ctx.line_col(pair_span.start).column,
                key_line,
                right_sibling_later,
            };
            self.eager_check_call_hash(ctx, &pair.value(), open_span, open_line, Some(fact));
        }
    }

    /// RuboCop's `hash_pair_where_value_beginning_with`/
    /// `right_sibling_begins_on_subsequent_line?`, recorded once for
    /// `elements`'s own direct pairs (a hash/keyword-hash literal's pairs
    /// are visited exactly once by the engine, so this never re-walks a
    /// subtree the traversal has already covered): for every pair whose
    /// value is itself an explicit-brace hash literal, stores that hash's
    /// `ParentPairFact` in `self.parent_facts`, keyed by the hash's own
    /// span, for `check`/`indent_base` to look up once the traversal
    /// reaches it.
    fn record_facts_one_level(&mut self, ctx: &Context<'_>, elements: &NodeList<'_>) {
        let items: Vec<Node<'_>> = elements.iter().collect();
        for (i, item) in items.iter().enumerate() {
            let Some(pair) = item.as_assoc_node() else { continue };
            let Some(hash) = pair.value().as_hash_node() else { continue };
            let pair_span = pair.location().span();
            let key_line = ctx.line_col(pair_span.start).line;
            let right_sibling_later = items.get(i + 1).is_some_and(|sib| {
                let last_line = last_line_of(ctx, pair_span);
                ctx.line_col(sib.span().start).line > last_line
            });
            let fact = ParentPairFact {
                column: ctx.line_col(pair_span.start).column,
                key_line,
                right_sibling_later,
            };
            self.parent_facts.insert(hash.location().span(), fact);
        }
    }

    /// RuboCop's `check`.
    fn check(
        &mut self,
        ctx: &mut Context<'_>,
        hash: &HashNode<'_>,
        left_paren: Option<Span>,
        parent_fact: Option<ParentPairFact>,
    ) {
        let left_brace = hash.opening_loc().span();
        let items: Vec<Node<'_>> = hash.elements().iter().collect();
        let pairs: Vec<AssocNode<'_>> = items.iter().filter_map(Node::as_assoc_node).collect();
        let has_first_pair = !pairs.is_empty();

        if let Some(first_pair) = pairs.first() {
            if same_line(ctx, first_pair.location().span().start, left_brace.start) {
                return;
            }
            if self.separator_style(first_pair) {
                self.check_based_on_longest_key(ctx, &pairs, left_brace, left_paren, parent_fact);
            } else {
                self.check_first(ctx, first_pair, left_brace, left_paren, 0, parent_fact);
            }
        }

        self.check_right_brace(
            ctx,
            hash.closing_loc().span(),
            left_brace,
            has_first_pair,
            left_paren,
            parent_fact,
        );
    }

    /// RuboCop's `separator_style?`.
    fn separator_style(&self, pair: &AssocNode<'_>) -> bool {
        if pair.operator_loc().is_some() {
            self.hash_rocket_separator
        } else {
            self.colon_separator
        }
    }

    /// RuboCop's `check_based_on_longest_key`.
    fn check_based_on_longest_key(
        &mut self,
        ctx: &mut Context<'_>,
        pairs: &[AssocNode<'_>],
        left_brace: Span,
        left_paren: Option<Span>,
        parent_fact: Option<ParentPairFact>,
    ) {
        let Some(first) = pairs.first() else { return };
        let lengths: Vec<i64> = pairs.iter().map(|p| char_len(ctx, p.key().span())).collect();
        let Some(&max) = lengths.iter().max() else { return };
        let offset = max - lengths[0];
        self.check_first(ctx, first, left_brace, left_paren, offset, parent_fact);
    }

    /// RuboCop's `check_first`, minus the `detected_styles`/
    /// `ambiguous_style_detected` bookkeeping (see `META.blind_spots`).
    #[allow(clippy::too_many_arguments)]
    fn check_first(
        &mut self,
        ctx: &mut Context<'_>,
        first: &AssocNode<'_>,
        left_brace: Span,
        left_paren: Option<Span>,
        offset: i64,
        parent_fact: Option<ParentPairFact>,
    ) {
        let first_span = first.location().span();
        let actual_column = i64::from(ctx.line_col(first_span.start).column);
        let (base_column, base_type) =
            self.indent_base(ctx, left_brace, true, parent_fact, left_paren);
        let expected_column = base_column + self.indentation_width + offset;
        let column_delta = expected_column - actual_column;
        if column_delta == 0 {
            return;
        }
        let msg = message(self.indentation_width, base_type.description());
        let taboo = heredoc_taboo(&first.as_node());
        match build_shift_fix(ctx, first_span, column_delta, &taboo) {
            Some(fix) => ctx.report_with_fix(&Self::META, first_span, msg, fix),
            None => ctx.report(&Self::META, first_span, msg),
        }
    }

    /// RuboCop's `check_right_brace`.
    #[allow(clippy::too_many_arguments)]
    fn check_right_brace(
        &mut self,
        ctx: &mut Context<'_>,
        right_brace: Span,
        left_brace: Span,
        has_first_pair: bool,
        left_paren: Option<Span>,
        parent_fact: Option<ParentPairFact>,
    ) {
        let line_col = ctx.line_col(right_brace.start);
        let line_text = ctx.line_text(line_col.line);
        if prefix_has_non_ws(line_text, line_col.column) {
            return;
        }
        let (base_column, base_type) =
            self.indent_base(ctx, left_brace, has_first_pair, parent_fact, left_paren);
        let column_delta = base_column - i64::from(line_col.column);
        if column_delta == 0 {
            return;
        }
        let msg = base_type.right_brace_message();
        match build_shift_fix(ctx, right_brace, column_delta, &[]) {
            Some(fix) => ctx.report_with_fix(&Self::META, right_brace, msg, fix),
            None => ctx.report(&Self::META, right_brace, msg),
        }
    }

    /// RuboCop's `indent_base`.
    fn indent_base(
        &self,
        ctx: &Context<'_>,
        left_brace: Span,
        has_first_pair: bool,
        parent_fact: Option<ParentPairFact>,
        left_paren: Option<Span>,
    ) -> (i64, BaseType) {
        if self.style == Style::AlignBraces {
            return (
                i64::from(ctx.line_col(left_brace.start).column),
                BaseType::LeftBraceOrBracket,
            );
        }
        if has_first_pair {
            if let Some(fact) = parent_fact {
                let brace_line = ctx.line_col(left_brace.start).line;
                if fact.key_line == brace_line && fact.right_sibling_later {
                    return (i64::from(fact.column), BaseType::ParentHashKey);
                }
            }
        }
        if let (Some(open), Style::SpecialInsideParentheses) = (left_paren, self.style) {
            return (
                i64::from(ctx.line_col(open.start).column) + 1,
                BaseType::FirstColumnAfterLeftParenthesis,
            );
        }
        let line_text = ctx.line_text(ctx.line_col(left_brace.start).line);
        (i64::from(first_non_ws_column(line_text)), BaseType::StartOfLine)
    }
}

/// The 1-based line of the last byte covered by `span`.
fn last_line_of(ctx: &Context<'_>, span: Span) -> u32 {
    ctx.line_col(span.end.saturating_sub(1).max(span.start)).line
}

fn same_line(ctx: &Context<'_>, a: u32, b: u32) -> bool {
    ctx.line_col(a).line == ctx.line_col(b).line
}

/// Ruby's `\s` character class, used by the `=~ /\S/` checks below.
fn is_ruby_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\x0B' | '\x0C')
}

/// RuboCop's `left_brace.source_line =~ /\S/`: the character column of the
/// first non-whitespace character on the line, or `0` if the line is
/// blank (never actually reached here since the line always holds at
/// least the `{`/`}` itself).
fn first_non_ws_column(line: &[u8]) -> u32 {
    match std::str::from_utf8(line) {
        Ok(text) => u32::try_from(text.chars().position(|c| !is_ruby_whitespace(c)).unwrap_or(0))
            .unwrap_or(0),
        Err(_) => 0,
    }
}

/// RuboCop's `right_brace.source_line[0...right_brace.column] =~ /\S/`.
fn prefix_has_non_ws(line: &[u8], upto_col: u32) -> bool {
    match std::str::from_utf8(line) {
        Ok(text) => text.chars().take(upto_col as usize).any(|c| !is_ruby_whitespace(c)),
        Err(_) => false,
    }
}

/// Ruby's `String#length` (character count) of the source text `span`
/// covers, for `hash_node.keys.map { |key| key.source_range.length }`.
fn char_len(ctx: &Context<'_>, span: Span) -> i64 {
    match std::str::from_utf8(ctx.text(span)) {
        Ok(text) => i64::try_from(text.chars().count()).unwrap_or(0),
        Err(_) => i64::from(span.len()),
    }
}

/// RuboCop's `AlignmentCorrector#inside_string_ranges`'s heredoc case:
/// collects heredoc body ranges within a subtree so autocorrection never
/// touches heredoc content (see `META.blind_spots` for the non-heredoc
/// delimited-literal case this does not cover).
struct HeredocTaboo {
    ranges: Vec<Span>,
}

impl<'pr> Visitor<'pr> for HeredocTaboo {
    fn enter(&mut self, node: &Node<'pr>) {
        let opening_closing = match node {
            Node::StringNode { .. } => {
                let n = node.as_string_node().expect("kind matched");
                n.opening_loc().zip(n.closing_loc())
            }
            Node::InterpolatedStringNode { .. } => {
                let n = node.as_interpolated_string_node().expect("kind matched");
                n.opening_loc().zip(n.closing_loc())
            }
            Node::XStringNode { .. } => {
                let n = node.as_x_string_node().expect("kind matched");
                Some((n.opening_loc(), n.closing_loc()))
            }
            Node::InterpolatedXStringNode { .. } => {
                let n = node.as_interpolated_x_string_node().expect("kind matched");
                Some((n.opening_loc(), n.closing_loc()))
            }
            _ => None,
        };
        if let Some((open, close)) = opening_closing {
            if open.as_slice().starts_with(b"<<") {
                self.ranges.push(Span::new(open.span().end, close.span().start));
            }
        }
    }
}

fn heredoc_taboo(node: &Node<'_>) -> Vec<Span> {
    let mut taboo = HeredocTaboo { ranges: Vec::new() };
    walk(node, &mut taboo);
    taboo.ranges
}

/// RuboCop's `AlignmentCorrector.correct`: shifts every physical line of
/// `span` by `column_delta` columns. Returns `None` when nothing could be
/// safely edited (a `=begin`/`=end` block comment inside the range, or
/// every line was blocked by a taboo range/whitespace mismatch).
fn build_shift_fix(
    ctx: &Context<'_>,
    span: Span,
    column_delta: i64,
    taboo: &[Span],
) -> Option<Fix> {
    let start_line = ctx.line_col(span.start).line;
    let last_byte = span.end.saturating_sub(1).max(span.start);
    let end_line = ctx.line_col(last_byte).line;

    for line in start_line..=end_line {
        if trim_start(ctx.line_text(line)).starts_with(b"=begin") {
            return None;
        }
    }

    let mut edits = Vec::new();
    for line in start_line..=end_line {
        let is_first = line == start_line;
        let anchor = if is_first { span.start } else { ctx.line_span(line).start };

        if column_delta > 0 {
            let amount = u32::try_from(column_delta).unwrap_or(0);
            if !is_first && ctx.line_span(line).is_empty() {
                continue;
            }
            if taboo.iter().any(|t| t.contains(Span::empty(anchor))) {
                continue;
            }
            edits.push(Edit::insert(anchor, " ".repeat(amount as usize).into_bytes()));
        } else {
            let amount = u32::try_from(-column_delta).unwrap_or(0);
            let starts_with_space =
                ctx.source().bytes().get(anchor as usize).is_some_and(|&b| b == b' ');
            let range = if is_first || !starts_with_space {
                Span::new(anchor.saturating_sub(amount), anchor)
            } else {
                Span::new(anchor, anchor + amount)
            };
            if taboo.iter().any(|t| t.contains(range)) {
                continue;
            }
            let text = ctx.text(range);
            if !text.is_empty() && text.iter().all(|&b| b == b' ' || b == b'\t') {
                edits.push(Edit::delete(range));
            }
        }
    }
    if edits.is_empty() {
        None
    } else {
        Some(Fix { applicability: Applicability::Safe, edits })
    }
}

fn trim_start(text: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < text.len() && text[i].is_ascii_whitespace() {
        i += 1;
    }
    &text[i..]
}

//! `Layout/FirstArrayElementIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/first_array_element_indentation.rb` plus the
//! `Alignment`, `ConfigurableEnforcedStyle`, and `MultilineElementIndentation`
//! mixins it includes.
//!
//! This mirrors `Layout/FirstHashElementIndentation`'s structure exactly
//! (see that cop's module doc comment for the full rationale), with the
//! checked literal kind swapped from hash to array:
//!
//! - **`left_parenthesis`** (RuboCop's `on_send`/`each_argument_node`/
//!   `on_node(:array, arg, :send)`): when an array literal argument's own
//!   `[` shares its source line with the enclosing call's `(`, indentation
//!   is measured from that call's opening parenthesis instead of from the
//!   start of the array's own line. [`FirstArrayElementIndentation::enter`]'s
//!   `CallNode` arm eagerly walks the call's own argument subtree (bounded
//!   to that one call, mirroring RuboCop's own bounded recursive `on_node`
//!   search) and, for every qualifying array, both marks it `ignored` (so
//!   the natural `ArrayNode` visit below does not double-check it) and
//!   resolves it immediately (`autocorrect_incompatible_with_other_cops?`
//!   then, unless skipped, `check`).
//! - **the `:parent_hash_key` basis** (RuboCop's
//!   `hash_pair_where_value_beginning_with`/
//!   `right_sibling_begins_on_subsequent_line?`): when an array literal is
//!   itself the value of a `key: [ ... ]` pair whose key starts on the same
//!   line as the array's own `[`, and that pair has a sibling pair starting
//!   on a later line, indentation is measured from the key's own column
//!   instead. This is a tree-shape fact about a hash/keyword-hash literal's
//!   own direct pairs, unrelated to arrays, so it is recorded once per
//!   hash/keyword-hash literal's own pairs as the engine naturally visits
//!   each one (`record_facts_one_level`, keyed by each pair's *value* span
//!   in `parent_facts`) and looked up by the value array when the traversal
//!   reaches it. The eager call-argument walk above duplicates this same
//!   one-level computation locally (`eager_check_pairs`) because it runs
//!   before the engine's own visit of the containing hash/keyword-hash has
//!   populated `parent_facts` for it.
//!
//! `autocorrect_incompatible_with_other_cops?` (RuboCop's `EnforcedStyle:
//! with_fixed_indentation` conflict with `Layout/ArrayAlignment`) is, unlike
//! `Layout/FirstHashElementIndentation`'s equivalent `Layout/ArgumentAlignment`
//! check, evaluated per array literal (using that literal's own would-be
//! `indent_base`) rather than as a single blanket `on_send` skip, so it is
//! threaded through both the natural `ArrayNode` visit and the eager
//! call-argument walk.

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::ArrayNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind, NodeList};
use ruby_source::{is_ruby_whitespace_char, Span};

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    SpecialInsideParentheses,
    Consistent,
    AlignBrackets,
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
            BaseType::LeftBraceOrBracket => "the position of the opening bracket",
            BaseType::FirstColumnAfterLeftParenthesis => {
                "the first position after the preceding left parenthesis"
            }
            BaseType::ParentHashKey => "the parent hash key",
            BaseType::StartOfLine => "the start of the line where the left square bracket is",
        }
    }

    /// RuboCop's `message_for_right_bracket`.
    fn right_bracket_message(self) -> &'static str {
        match self {
            BaseType::LeftBraceOrBracket => {
                "Indent the right bracket the same as the left bracket."
            }
            BaseType::FirstColumnAfterLeftParenthesis => {
                "Indent the right bracket the same as the first position after the preceding \
                 left parenthesis."
            }
            BaseType::ParentHashKey => "Indent the right bracket the same as the parent hash key.",
            BaseType::StartOfLine => {
                "Indent the right bracket the same as the start of the line where the left \
                 bracket is."
            }
        }
    }
}

/// RuboCop's `MSG`.
fn message(width: i64, base_description: &str) -> String {
    format!("Use {width} spaces for indentation in an array, relative to {base_description}.")
}

/// RuboCop's `hash_pair_where_value_beginning_with`/
/// `right_sibling_begins_on_subsequent_line?`, precomputed for an array
/// literal that is the value of some pair, keyed by that array's own span.
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

/// Checks the indentation of the first element in an array literal, ported
/// from RuboCop's `FirstArrayElementIndentation` cop plus its `Alignment`,
/// `ConfigurableEnforcedStyle`, and `MultilineElementIndentation` mixins.
#[derive(Debug, Clone)]
pub struct FirstArrayElementIndentation {
    style: Style,
    indentation_width: i64,
    /// `Layout/ArrayAlignment`'s `EnforcedStyle == 'with_fixed_indentation'`.
    array_alignment_fixed: bool,
    /// Array literals already `check`ed eagerly from a governing call's
    /// argument list (RuboCop's `ignored_node?`).
    ignored: HashSet<Span>,
    /// `ParentPairFact`s recorded for array literals reachable as a plain
    /// pair value, keyed by the array's own span.
    parent_facts: HashMap<Span, ParentPairFact>,
}

impl Rule for FirstArrayElementIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/FirstArrayElementIndentation",
        department: Department::Layout,
        summary: "Checks the indentation of the first element in an array literal.",
        explanation: "\
Checks the indentation of the first element in an array literal where the
opening bracket and the first element are on separate lines. The other
elements' indentations are handled by `Layout/ArrayAlignment` cop.

This cop will respect `Layout/ArrayAlignment` and will not work when
`EnforcedStyle: with_fixed_indentation` is specified for `Layout/ArrayAlignment`.

By default, array literals that are arguments in a method call with
parentheses, and where the opening square bracket of the array is on the
same line as the opening parenthesis of the method call, shall have their
first element indented one step (two spaces) more than the position inside
the opening parenthesis.

Other array literals shall have their first element indented one step more
than the start of the line where the opening square bracket is.

This default style is called `special_inside_parentheses`.

```ruby
# EnforcedStyle: special_inside_parentheses (default)

# bad
array = [
  :value
]
and_in_a_method_call([
  :no_difference
                     ])

# good
array = [
  :value
]
but_in_a_method_call([
                        :its_like_this
                      ])
```

```ruby
# EnforcedStyle: consistent

# bad
array = [
  :value
]
but_in_a_method_call([
                        :its_like_this
])

# good
array = [
  :value
]
and_in_a_method_call([
  :no_difference
])
```

```ruby
# EnforcedStyle: align_brackets

# bad
and_now_for_something = [
                          :completely_different
]

# good
and_now_for_something = [
                          :completely_different
                        ]
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ArrayNode,
            NodeKind::HashNode,
            NodeKind::KeywordHashNode,
            NodeKind::CallNode,
        ],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("special_inside_parentheses"),
                allowed: &["special_inside_parentheses", "consistent", "align_brackets"],
                doc: "Whether an array literal argument's first element is indented relative \
                      to the preceding left parenthesis when the bracket shares its line \
                      (`special_inside_parentheses`), always relative to the start of the \
                      array's own line (`consistent`), or relative to the opening bracket's \
                      own column (`align_brackets`).",
            },
            ConfigOption {
                name: "IndentationWidth",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "Number of spaces for the first element's indentation, overriding \
                      `Layout/IndentationWidth`'s `Width` (which itself defaults to 2).",
            },
        ],
        blind_spots: "\
RuboCop's `each_argument_node` finds array arguments with \
`on_node(:array, arg, :send)`. Prism has no separate `block`/`csend` nodes, \
so this port treats a plain call as opaque except for its attached `do`/`{}` \
block, and descends into `&.` calls, which is how parser's tree shapes \
`on_node` in those cases.

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
            "align_brackets" => Style::AlignBrackets,
            _ => Style::SpecialInsideParentheses,
        };
        let indentation_width = options
            .get("IndentationWidth")
            .and_then(OptionValue::as_int)
            .or_else(|| {
                options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
            })
            .unwrap_or(2);
        let array_alignment_fixed =
            options.peer("Layout/ArrayAlignment", "EnforcedStyle").and_then(OptionValue::as_str)
                == Some("with_fixed_indentation");
        Ok(Self {
            style,
            indentation_width,
            array_alignment_fixed,
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
            NodeKind::ArrayNode => {
                let array = node.as_array_node().expect("kind matched");
                let span = array.location().span();
                if array.opening_loc().is_some() && !self.ignored.contains(&span) {
                    let fact = self.parent_facts.get(&span).copied();
                    if !self.autocorrect_incompatible(ctx, &array, None, fact) {
                        self.check(ctx, &array, None, fact);
                    }
                }
            }
            NodeKind::HashNode => {
                let hash = node.as_hash_node().expect("kind matched");
                self.record_facts_one_level(ctx, &hash.elements());
            }
            NodeKind::KeywordHashNode => {
                let kw = node.as_keyword_hash_node().expect("kind matched");
                self.record_facts_one_level(ctx, &kw.elements());
            }
            NodeKind::CallNode => {
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
                    self.eager_check_call_array(ctx, &arg, open_span, open_line, None);
                }
            }
            _ => {}
        }
    }
}

impl FirstArrayElementIndentation {
    /// RuboCop's `each_argument_node`/`on_node(:array, arg, :send)`: finds
    /// every explicit-bracket array literal reachable from `node` without
    /// crossing into a nested call's own arguments (see `META.blind_spots`
    /// for how parser's `block`/`csend` shapes map onto Prism), and -- for
    /// each one whose own opening bracket shares `open_line` with the
    /// enclosing call's opening parenthesis -- resolves it eagerly with
    /// `parent_fact` as its precomputed `:parent_hash_key` basis, marking it
    /// `ignored` so the natural `ArrayNode` visit does not double-check it.
    fn eager_check_call_array(
        &mut self,
        ctx: &mut Context<'_>,
        node: &Node<'_>,
        open_span: Span,
        open_line: u32,
        parent_fact: Option<ParentPairFact>,
    ) {
        if let Some(array) = node.as_array_node() {
            if let Some(opening) = array.opening_loc() {
                if ctx.line_col(opening.span().start).line == open_line {
                    self.ignored.insert(array.location().span());
                    if !self.autocorrect_incompatible(ctx, &array, Some(open_span), parent_fact) {
                        self.check(ctx, &array, Some(open_span), parent_fact);
                    }
                }
            }
            for el in &array.elements() {
                self.eager_check_call_array(ctx, &el, open_span, open_line, None);
            }
            return;
        }
        if let Some(hash) = node.as_hash_node() {
            self.eager_check_pairs(ctx, &hash.elements(), open_span, open_line);
            return;
        }
        if let Some(kw) = node.as_keyword_hash_node() {
            self.eager_check_pairs(ctx, &kw.elements(), open_span, open_line);
            return;
        }
        // `on_node(:array, arg, :send)` descends through every other node
        // type and stops only at a `send`. Prism folds parser's `block`
        // wrapper into its call and `csend` into `CallNode`, so: a plain call
        // is opaque except for its block (parser's `block` node, whose
        // `send` child is the excluded part), and a `&.` call is descended.
        let mut children = Vec::new();
        match node.as_call_node() {
            Some(call) if !call.is_safe_navigation() => {
                if let Some(block) = call.block().filter(|b| b.as_block_node().is_some()) {
                    ruby_ast::for_each_child(&block, |child| children.push(*child));
                }
            }
            _ => ruby_ast::for_each_child(node, |child| children.push(*child)),
        }
        for child in &children {
            self.eager_check_call_array(ctx, child, open_span, open_line, None);
        }
    }

    /// One level of `eager_check_call_array`'s pair walk: computes each
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
                let last_line = ctx.last_line(pair_span);
                ctx.line_col(sib.span().start).line > last_line
            });
            let fact = ParentPairFact {
                column: ctx.line_col(pair_span.start).column,
                key_line,
                right_sibling_later,
            };
            self.eager_check_call_array(ctx, &pair.value(), open_span, open_line, Some(fact));
        }
    }

    /// RuboCop's `hash_pair_where_value_beginning_with`/
    /// `right_sibling_begins_on_subsequent_line?`, recorded once for
    /// `elements`'s own direct pairs (a hash/keyword-hash literal's pairs
    /// are visited exactly once by the engine, so this never re-walks a
    /// subtree the traversal has already covered): for every pair whose
    /// value is itself an explicit-bracket array literal, stores that
    /// array's `ParentPairFact` in `self.parent_facts`, keyed by the
    /// array's own span, for `check`/`indent_base` to look up once the
    /// traversal reaches it.
    fn record_facts_one_level(&mut self, ctx: &Context<'_>, elements: &NodeList<'_>) {
        let items: Vec<Node<'_>> = elements.iter().collect();
        for (i, item) in items.iter().enumerate() {
            let Some(pair) = item.as_assoc_node() else { continue };
            let Some(array) = pair.value().as_array_node() else { continue };
            let pair_span = pair.location().span();
            let key_line = ctx.line_col(pair_span.start).line;
            let right_sibling_later = items.get(i + 1).is_some_and(|sib| {
                let last_line = ctx.last_line(pair_span);
                ctx.line_col(sib.span().start).line > last_line
            });
            let fact = ParentPairFact {
                column: ctx.line_col(pair_span.start).column,
                key_line,
                right_sibling_later,
            };
            self.parent_facts.insert(array.location().span(), fact);
        }
    }

    /// RuboCop's `autocorrect_incompatible_with_other_cops?`.
    fn autocorrect_incompatible(
        &self,
        ctx: &Context<'_>,
        array: &ArrayNode<'_>,
        left_paren: Option<Span>,
        parent_fact: Option<ParentPairFact>,
    ) -> bool {
        if !self.array_alignment_fixed {
            return false;
        }
        if self.style != Style::Consistent {
            return true;
        }
        // `Layout/ArrayAlignment` does not align single-element arrays.
        let elements = array.elements();
        if elements.iter().count() < 2 {
            return false;
        }
        let Some(left_bracket) = array.opening_loc() else { return false };
        let (_, base_type) =
            self.indent_base(ctx, left_bracket.span(), true, parent_fact, left_paren);
        base_type != BaseType::StartOfLine
    }

    /// RuboCop's `check`.
    fn check(
        &mut self,
        ctx: &mut Context<'_>,
        array: &ArrayNode<'_>,
        left_paren: Option<Span>,
        parent_fact: Option<ParentPairFact>,
    ) {
        let Some(left_bracket_loc) = array.opening_loc() else { return };
        let left_bracket = left_bracket_loc.span();
        let items: Vec<Node<'_>> = array.elements().iter().collect();
        let first_elem = items.first();

        if let Some(first) = first_elem {
            if ctx.same_line(Span::empty(first.span().start), Span::empty(left_bracket.start)) {
                return;
            }
            self.check_first(ctx, first, left_bracket, left_paren, parent_fact);
        }

        let Some(right_bracket_loc) = array.closing_loc() else { return };
        self.check_right_bracket(
            ctx,
            right_bracket_loc.span(),
            left_bracket,
            first_elem.is_some(),
            left_paren,
            parent_fact,
        );
    }

    /// RuboCop's `check_first`, minus the `detected_styles`/
    /// `ambiguous_style_detected` bookkeeping (see `META.blind_spots`).
    fn check_first(
        &mut self,
        ctx: &mut Context<'_>,
        first: &Node<'_>,
        left_bracket: Span,
        left_paren: Option<Span>,
        parent_fact: Option<ParentPairFact>,
    ) {
        let first_span = first.span();
        let actual_column = i64::from(ctx.line_col(first_span.start).column);
        let (base_column, base_type) =
            self.indent_base(ctx, left_bracket, true, parent_fact, left_paren);
        let expected_column = base_column + self.indentation_width;
        let column_delta = expected_column - actual_column;
        if column_delta == 0 {
            return;
        }
        let msg = message(self.indentation_width, base_type.description());
        let taboo = linter::heredoc_bodies(ctx, first);
        let delta = i32::try_from(column_delta).unwrap_or(0);
        let edits = linter::shift_lines(ctx, first_span, delta, &taboo);
        match fix_from_edits(edits) {
            Some(fix) => ctx.report_with_fix(&Self::META, first_span, msg, fix),
            None => ctx.report(&Self::META, first_span, msg),
        }
    }

    /// RuboCop's `check_right_bracket`.
    #[allow(clippy::too_many_arguments)]
    fn check_right_bracket(
        &mut self,
        ctx: &mut Context<'_>,
        right_bracket: Span,
        left_bracket: Span,
        has_first_elem: bool,
        left_paren: Option<Span>,
        parent_fact: Option<ParentPairFact>,
    ) {
        let line_col = ctx.line_col(right_bracket.start);
        let line_text = ctx.line_text(line_col.line);
        if prefix_has_non_ws(line_text, line_col.column) {
            return;
        }
        let (base_column, base_type) =
            self.indent_base(ctx, left_bracket, has_first_elem, parent_fact, left_paren);
        let column_delta = base_column - i64::from(line_col.column);
        if column_delta == 0 {
            return;
        }
        let msg = base_type.right_bracket_message();
        let delta = i32::try_from(column_delta).unwrap_or(0);
        match fix_from_edits(linter::shift_lines(ctx, right_bracket, delta, &[])) {
            Some(fix) => ctx.report_with_fix(&Self::META, right_bracket, msg, fix),
            None => ctx.report(&Self::META, right_bracket, msg),
        }
    }

    /// RuboCop's `indent_base`.
    fn indent_base(
        &self,
        ctx: &Context<'_>,
        left_bracket: Span,
        has_first_elem: bool,
        parent_fact: Option<ParentPairFact>,
        left_paren: Option<Span>,
    ) -> (i64, BaseType) {
        if self.style == Style::AlignBrackets {
            return (
                i64::from(ctx.line_col(left_bracket.start).column),
                BaseType::LeftBraceOrBracket,
            );
        }
        if has_first_elem {
            if let Some(fact) = parent_fact {
                let bracket_line = ctx.line_col(left_bracket.start).line;
                if fact.key_line == bracket_line && fact.right_sibling_later {
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
        let line_text = ctx.line_text(ctx.line_col(left_bracket.start).line);
        (i64::from(first_non_ws_column(line_text)), BaseType::StartOfLine)
    }
}

/// RuboCop's `left_bracket.source_line =~ /\S/`: the character column of the
/// first non-whitespace character on the line, or `0` if the line is blank
/// (never actually reached here since the line always holds at least the
/// `[`/`]` itself).
fn first_non_ws_column(line: &[u8]) -> u32 {
    match std::str::from_utf8(line) {
        Ok(text) => {
            u32::try_from(text.chars().position(|c| !is_ruby_whitespace_char(c)).unwrap_or(0))
                .unwrap_or(0)
        }
        Err(_) => 0,
    }
}

/// RuboCop's `right_bracket.source_line[0...right_bracket.column] =~ /\S/`.
fn prefix_has_non_ws(line: &[u8], upto_col: u32) -> bool {
    match std::str::from_utf8(line) {
        Ok(text) => text.chars().take(upto_col as usize).any(|c| !is_ruby_whitespace_char(c)),
        Err(_) => false,
    }
}

/// Wraps a possibly-empty edit list from [`linter::shift_lines`] into the
/// `Option<Fix>` shape this cop's call sites report with.
fn fix_from_edits(edits: Vec<Edit>) -> Option<Fix> {
    if edits.is_empty() {
        None
    } else {
        Some(Fix { applicability: Applicability::Safe, edits })
    }
}

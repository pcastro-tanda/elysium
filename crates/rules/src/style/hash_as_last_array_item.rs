//! `Style/HashAsLastArrayItem`, ported from RuboCop's
//! `lib/rubocop/cop/style/hash_as_last_array_item.rb`.
//!
//! Prism gives a braced hash literal (`{ ... }`) its own [`NodeKind::HashNode`]
//! and a braceless bare-keyword hash (`one: 1, two: 2`, no `{}`) its own
//! [`NodeKind::KeywordHashNode`] -- RuboCop-AST's single `hash` node type,
//! split by `braces?`/`!braces?` (whitequark's `hash_type?`). Prism nodes
//! carry no `parent` pointer, so instead of upstream's `on_hash` (which walks
//! up to `containing_array`), this rule walks [`NodeKind::ArrayNode`]
//! directly and inspects only the array's last element -- the only position
//! `expected_braced_last_array_item?` can ever flag.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Braces,
    NoBraces,
}

/// Checks for presence or absence of braces around hash literal as a last array item depending on configuration.
#[derive(Debug, Clone)]
pub struct HashAsLastArrayItem {
    style: Style,
}

impl Rule for HashAsLastArrayItem {
    const META: RuleMeta = RuleMeta {
        name: "Style/HashAsLastArrayItem",
        department: Department::Style,
        summary: "Checks for presence or absence of braces around hash literal as a last array item depending on configuration.",
        explanation: "\
Checks for presence or absence of braces around hash literal as a last
array item depending on configuration.

NOTE: This cop will ignore arrays where multiple items are all hashes,
regardless of `EnforcedStyle`.

```ruby
[{ one: 1 }, { two: 2 }]
```

```ruby
# EnforcedStyle: braces (default)

# bad
[1, 2, one: 1, two: 2]

# good
[1, 2, { one: 1, two: 2 }]

# bad
[one: 1, two: 2]

# good
[{ one: 1, two: 2 }]
```

```ruby
# EnforcedStyle: no_braces

# bad
[1, 2, { one: 1, two: 2 }]

# good
[1, 2, one: 1, two: 2]

# bad
[{ one: 1, two: 2 }]

# good
[one: 1, two: 2]
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("braces"),
            allowed: &["braces", "no_braces"],
            doc: "Whether a hash literal as the last array item should be wrapped in braces.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "no_braces" => Style::NoBraces,
            _ => Style::Braces,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(array) = node.as_array_node() else { return };
        // RuboCop's `explicit_array?`: an implicit array (no `[`/`]`) cannot
        // have an "unbraced" hash.
        if array.opening_loc().is_none() {
            return;
        }
        let elements: Vec<Node<'_>> = array.elements().iter().collect();
        let Some(last) = elements.last() else { return };
        let Some(last_braced) = hash_braces(last) else { return };

        // RuboCop's `on_hash`'s `return if node.children.first&.kwsplat_type?`.
        if hash_elements(last).first().is_some_and(|el| el.as_assoc_splat_node().is_some()) {
            return;
        }

        if !expected_braced_last_array_item(self.style, &elements) {
            return;
        }

        match self.style {
            Style::Braces => check_braces(last, node, last_braced, ctx),
            Style::NoBraces => check_no_braces(last, last_braced, ctx),
        }
    }
}

/// RuboCop-AST's `braces?`: `Some(true)` for a `HashNode` (`{...}`),
/// `Some(false)` for a `KeywordHashNode` (bare `key: value`), `None` when
/// `node` isn't a hash at all.
fn hash_braces(node: &Node<'_>) -> Option<bool> {
    if node.as_hash_node().is_some() {
        Some(true)
    } else if node.as_keyword_hash_node().is_some() {
        Some(false)
    } else {
        None
    }
}

/// `HashNode`/`KeywordHashNode#children` (Prism: `#elements`). Empty when
/// `node` isn't a hash at all.
fn hash_elements<'pr>(node: &Node<'pr>) -> Vec<Node<'pr>> {
    if let Some(h) = node.as_hash_node() {
        h.elements().iter().collect()
    } else if let Some(h) = node.as_keyword_hash_node() {
        h.elements().iter().collect()
    } else {
        Vec::new()
    }
}

/// RuboCop's `expected_braced_last_array_item?`.
fn expected_braced_last_array_item(style: Style, elements: &[Node<'_>]) -> bool {
    let wants_braces = matches!(style, Style::Braces);
    let all_match_style =
        elements.iter().all(|el| hash_braces(el).is_some_and(|braced| braced == wants_braces));
    if all_match_style {
        return false;
    }
    let second_to_last_is_hash =
        elements.len() >= 2 && hash_braces(&elements[elements.len() - 2]).is_some();
    !second_to_last_is_hash
}

/// RuboCop's `check_braces`.
fn check_braces(hash: &Node<'_>, array: &Node<'_>, braced: bool, ctx: &mut Context<'_>) {
    if braced {
        return;
    }
    let span = hash.span();
    let edits = if ctx.is_single_line(span) || ctx.same_line(span, array.span()) {
        vec![Edit::insert(span.start, b"{".to_vec()), Edit::insert(span.end, b"}".to_vec())]
    } else {
        let indent = " ".repeat(ctx.line_col(span.start).column as usize);
        let before = format!("{{\n{indent}");
        let after = format!("\n{indent}}}");
        vec![
            Edit::insert(span.start, before.into_bytes()),
            Edit::insert(span.end, after.into_bytes()),
        ]
    };
    let fix = Fix { applicability: Applicability::Safe, edits };
    ctx.report_with_fix(&HashAsLastArrayItem::META, span, "Wrap hash in `{` and `}`.", fix);
}

/// RuboCop's `check_no_braces`.
fn check_no_braces(hash: &Node<'_>, braced: bool, ctx: &mut Context<'_>) {
    if !braced {
        return;
    }
    let h = hash.as_hash_node().expect("braced implies HashNode");
    if h.elements().is_empty() {
        return; // Empty hash cannot be "unbraced".
    }
    let span = hash.span();
    let mut edits =
        vec![Edit::delete(h.opening_loc().span()), Edit::delete(h.closing_loc().span())];
    if let Some(comma) = trailing_comma_span(ctx, span.end) {
        edits.push(Edit::delete(comma));
    }
    let fix = Fix { applicability: Applicability::Safe, edits };
    ctx.report_with_fix(&HashAsLastArrayItem::META, span, "Omit the braces around the hash.", fix);
}

/// RuboCop's `remove_last_element_trailing_comma`: starting at `pos`, skips
/// spaces/tabs then newlines (`range_with_surrounding_space(side: :right)`'s
/// default passes), and returns the single following byte's span if it is a
/// `,`.
fn trailing_comma_span(ctx: &Context<'_>, mut pos: u32) -> Option<Span> {
    let src = ctx.source().bytes();
    while src.get(pos as usize).is_some_and(|&b| b == b' ' || b == b'\t') {
        pos += 1;
    }
    while src.get(pos as usize) == Some(&b'\n') {
        pos += 1;
    }
    (src.get(pos as usize) == Some(&b',')).then(|| Span::new(pos, pos + 1))
}

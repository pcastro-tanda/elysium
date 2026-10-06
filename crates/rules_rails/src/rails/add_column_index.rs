//! `Rails/AddColumnIndex`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/add_column_index.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

const MSG: &str = "`add_column` does not accept an `index` key, use `add_index` instead.";

/// Checks for migrations using `add_column` that have an `index` key.
#[derive(Debug, Clone)]
pub struct AddColumnIndex;

impl Rule for AddColumnIndex {
    const META: RuleMeta = RuleMeta {
        name: "Rails/AddColumnIndex",
        department: Department::Rails,
        summary: "Rails migrations don't make use of a given `index` key, but also doesn't \
                  given an error when it's used, so it makes it seem like an index might be \
                  used.",
        explanation: "Checks for migrations using `add_column` that have an `index` key. \
                      `add_column` does not accept `index`, but also does not raise an error \
                      for extra keys, so it is possible to mistakenly add the key without \
                      realizing it will not actually add an index.\n\n```ruby\n# bad (will not \
                      add an index)\nadd_column :table, :column, :integer, index: true\n\n# \
                      good\nadd_column :table, :column, :integer\nadd_index :table, :column\n\
                      ```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"add_column"
            || call.receiver().is_some()
            || call.is_safe_navigation()
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let arguments: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let [table, column, rest @ ..] = arguments.as_slice() else { return };

        let Some((pair, value)) = rest.iter().find_map(index_pair) else { return };
        let pair_span = pair.span();

        let removal = index_range(ctx, pair_span);
        let call_end = call_span_excluding_block(&call).end;
        let mut add_index = Vec::new();
        add_index.extend_from_slice(b"\nadd_index ");
        add_index.extend_from_slice(ctx.text(table.span()));
        add_index.extend_from_slice(b", ");
        add_index.extend_from_slice(ctx.text(column.span()));
        if let Some(hash) = value.as_hash_node() {
            // `value.source_range.adjust(begin_pos: 1, end_pos: -1).source.strip`
            let span = hash.as_node().span();
            let inner = ctx.text(Span::new(span.start + 1, span.end - 1));
            add_index.extend_from_slice(b", ");
            add_index.extend_from_slice(inner.trim_ascii());
        }

        ctx.report_with_fix(
            &Self::META,
            pair_span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::delete(removal), Edit::insert(call_end, add_index)],
            },
        );
    }
}

/// `<(hash <$(pair {(sym :index) (str "index")} $_) ...>) ...>`: the first
/// `index` pair of a hash argument, with its value.
fn index_pair<'pr>(argument: &Node<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    let elements = if let Some(hash) = argument.as_hash_node() {
        hash.elements()
    } else {
        argument.as_keyword_hash_node()?.elements()
    };
    elements.iter().find_map(|element| {
        let assoc = element.as_assoc_node()?;
        let key = assoc.key();
        let is_index = if let Some(symbol) = key.as_symbol_node() {
            symbol.unescaped() == b"index"
        } else {
            key.as_string_node().is_some_and(|string| string.unescaped() == b"index")
        };
        is_index.then(|| (element, assoc.value()))
    })
}

/// `range_with_surrounding_comma(range_with_surrounding_space(range, side:
/// :left), :left)`.
fn index_range(ctx: &Context<'_>, pair: Span) -> Span {
    let spaced = ctx.with_surrounding_space(pair, Side::Left, true, false);
    let preceded_by_comma =
        spaced.start > 0 && ctx.source().bytes()[spaced.start as usize - 1] == b',';
    Span::new(spaced.start - u32::from(preceded_by_comma), spaced.end)
}

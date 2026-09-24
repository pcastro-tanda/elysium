//! `Style/TrailingCommaInArguments`, ported from RuboCop's
//! `lib/rubocop/cop/style/trailing_comma_in_arguments.rb` (shared logic:
//! `lib/rubocop/cop/mixin/trailing_comma.rb`, ported in `super::trailing_comma`).

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind, NodeList};
use ruby_source::Span;

use super::trailing_comma::{self, MultilineStyle, TrailingCommaNode};

/// RuboCop's `kind` argument to `check`: `'parameter of %<article>s method call'`.
const KIND: &str = "parameter of %<article>s method call";

/// Checks for trailing comma in argument lists.
#[derive(Debug, Clone)]
pub struct TrailingCommaInArguments {
    style: MultilineStyle,
}

impl Rule for TrailingCommaInArguments {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrailingCommaInArguments",
        department: Department::Style,
        summary: "Checks for trailing comma in argument lists.",
        explanation: "\
Regardless of style, trailing commas are not allowed in single-line method
calls.

```ruby
# EnforcedStyleForMultiline: consistent_comma
# bad
method(1, 2,)

# good
method(1, 2)

# good
method(
  1, 2,
  3,
)

# good
method(
  1, 2, 3,
)

# good
method(
  1,
  2,
)
```

```ruby
# EnforcedStyleForMultiline: comma
# bad
method(1, 2,)

# good
method(1, 2)

# bad
method(
  1, 2,
  3,
)

# good
method(
  1, 2,
  3
)

# bad
method(
  1, 2, 3,
)

# good
method(
  1, 2, 3
)

# good
method(
  1,
  2,
)
```

```ruby
# EnforcedStyleForMultiline: diff_comma
# bad
method(1, 2,)

# good
method(1, 2)

# good
method(
  1, 2,
  3,
)

# good
method(
  1, 2, 3,
)

# good
method(
  1,
  2,
)

# bad
method(1, [
  2,
],)

# good
method(1, [
  2,
])

# bad
object[1, 2,
       3, 4,]

# good
object[1, 2,
       3, 4]
```

```ruby
# EnforcedStyleForMultiline: no_comma (default)
# bad
method(1, 2,)

# bad
object[1, 2,]

# good
method(1, 2)

# good
object[1, 2]

# good
method(
  1,
  2
)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyleForMultiline",
            default: ConfigDefault::Str("no_comma"),
            allowed: &["comma", "consistent_comma", "diff_comma", "no_comma"],
            doc: "Trailing comma style for multiline parenthesized/`[]` method calls.",
        }],
        blind_spots: "\
Only parenthesized calls and index reads (`foo(...)`/`foo[...]`) are checked,
matching RuboCop's own `on_send` guard; unparenthesized calls, `super`,
`yield`, and index *assignment* (`foo[...] = ...`, a different method name in
Prism) are never checked, since RuboCop doesn't check them either.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = options.style("EnforcedStyleForMultiline")?;
        Ok(Self { style: MultilineStyle::parse(style) })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::CallNode { .. } = node else { return };
        let call = node.as_call_node().expect("kind matched");

        let Some(arguments) = call.arguments() else { return };
        let args_list = arguments.arguments();
        if args_list.is_empty() {
            return;
        }

        let parenthesized =
            call.opening_loc().is_some_and(|o| ctx.text(o.span()).first() == Some(&b'('));
        let is_index = call.name().as_slice() == b"[]";
        if !parenthesized && !is_index {
            return;
        }
        let Some(closing) = call.closing_loc() else { return };
        let closing_span = closing.span();

        // RuboCop/whitequark's `send` AST folds an explicit `&blk`-style block-pass argument
        // into the trailing position of `node.arguments` (`node.last_argument` is the
        // `block_pass` node, so `put_comma`/`should_have_comma?` treat it as the call's last
        // item). Prism instead exposes it via `CallNode#block` as a `BlockArgumentNode`,
        // separate from `arguments()` - recover the same "last argument" view here. A `do...end`
        // /`{}` block is a `BlockNode` and is left alone: RuboCop's `send` node never includes it
        // either.
        let block_pass = call.block().filter(|b| matches!(b, Node::BlockArgumentNode { .. }));
        let last_item_is_block_pass = block_pass.is_some();
        let last_raw_span = block_pass.as_ref().map_or_else(
            || args_list.last().expect("checked non-empty").location().span(),
            |bp| bp.location().span(),
        );
        let node_span = Span::new(node.location().span().start, closing_span.end);
        let selector_line = Some(call.message_loc().map_or_else(
            || ctx.line_col(node_span.start).line,
            |m| ctx.line_col(m.span().start).line,
        ));
        let last_is_braced_hash = block_pass.is_none()
            && matches!(args_list.last().expect("checked non-empty"), Node::HashNode { .. });
        let any_heredoc = trailing_comma::any_heredoc(ctx, args_list.iter())
            || block_pass.as_ref().is_some_and(|bp| trailing_comma::is_heredoc(ctx, bp));
        let mut elements = elements(ctx, &args_list);
        if let Some(bp) = &block_pass {
            elements.push(bp.location().span());
        }

        let target = TrailingCommaNode {
            elements,
            node_span,
            closing: closing_span,
            last_item: last_raw_span,
            last_item_is_block_pass,
            any_heredoc,
            selector_line,
            last_is_braced_hash,
        };
        trailing_comma::check(ctx, &Self::META, self.style, KIND, &target);
    }
}

/// RuboCop's `elements(node)` for call arguments: every multiline braceless-hash argument
/// (`KeywordHashNode`) is promoted to its own pairs, for the purposes of the multiline-ness checks
/// only; every other argument (including a braced hash literal) counts as a single element.
fn elements(ctx: &Context<'_>, args: &NodeList<'_>) -> Vec<Span> {
    let mut out = Vec::with_capacity(args.len());
    for arg in args {
        if let Node::KeywordHashNode { .. } = &arg {
            let kw = arg.as_keyword_hash_node().expect("kind matched");
            let span = kw.location().span();
            if trailing_comma::is_multiline_span(ctx, span) {
                out.extend(kw.elements().iter().map(|el| el.location().span()));
                continue;
            }
        }
        out.push(arg.location().span());
    }
    out
}

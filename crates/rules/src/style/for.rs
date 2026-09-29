//! `Style/For`, ported from RuboCop's
//! `lib/rubocop/cop/style/for.rb`, together with its
//! `ForToEachCorrector`/`EachToForCorrector` autocorrect mixins.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode, ForNode, ParametersNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const PREFER_EACH: &str = "Prefer `each` over `for`.";
const PREFER_FOR: &str = "Prefer `for` over `each`.";

/// RuboCop's `MethodIdentifierPredicates::OPERATOR_METHODS`.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Each,
    For,
}

/// Checks use of for or each in multiline loops.
#[derive(Debug, Clone)]
pub struct For {
    style: Style,
}

impl Rule for For {
    const META: RuleMeta = RuleMeta {
        name: "Style/For",
        department: Department::Style,
        summary: "Checks use of for or each in multiline loops.",
        explanation: "\
Looks for uses of the `for` keyword or `each` method. The preferred \
alternative is set in the `EnforcedStyle` configuration parameter. An \
`each` call with a block on a single line is always allowed.

NOTE: `each` is preferred in idiomatic Ruby because `for` leaks its loop \
variable into the surrounding scope.

```ruby
# EnforcedStyle: each (default)
# bad
def foo
  for n in [1, 2, 3] do
    puts n
  end
end

# good
def foo
  [1, 2, 3].each do |n|
    puts n
  end
end
```

```ruby
# EnforcedStyle: for
# bad
def foo
  [1, 2, 3].each do |n|
    puts n
  end
end

# good
def foo
  for n in [1, 2, 3] do
    puts n
  end
end
```

This cop's autocorrection is unsafe because the scope of variables is \
different between `each` and `for`.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ForNode, NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("each"),
            allowed: &["each", "for"],
            doc: "Whether to prefer `for` or `each` for multiline loops.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "for" => Style::For,
            _ => Style::Each,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ForNode => self.check_for(node, ctx),
            NodeKind::CallNode => self.check_each(node, ctx),
            _ => {}
        }
    }
}

impl For {
    fn check_for(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.style != Style::Each {
            return;
        }
        let for_node = node.as_for_node().expect("kind matched");
        let edit = for_to_each_edit(ctx, &for_node);
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            PREFER_EACH,
            Fix { applicability: Applicability::Unsafe, edits: vec![edit] },
        );
    }

    fn check_each(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.style != Style::For {
            return;
        }
        let call = node.as_call_node().expect("kind matched");
        let Some(block_node) = call.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };
        if !suspect_enumerable(&call, &block, ctx) {
            return;
        }
        if call.receiver().is_none() {
            return;
        }
        if rescue_or_ensure_body(&block) {
            return;
        }
        let edit = each_to_for_edit(ctx, node, &call, &block);
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            PREFER_FOR,
            Fix { applicability: Applicability::Unsafe, edits: vec![edit] },
        );
    }
}

/// RuboCop's `For#suspect_enumerable?`.
fn suspect_enumerable(call: &CallNode<'_>, block: &BlockNode<'_>, ctx: &Context<'_>) -> bool {
    !ctx.is_single_line(block.location().span())
        && call.name().as_slice() == b"each"
        && call.arguments().is_none_or(|a| a.arguments().is_empty())
}

/// RuboCop's `For#rescue_or_ensure_body?`: a def/block body with a top-level
/// `rescue`/`ensure`/`else` is an implicit `BeginNode` in Prism (see the
/// porting kit's whitequark trap notes).
fn rescue_or_ensure_body(block: &BlockNode<'_>) -> bool {
    block.body().is_some_and(|b| b.kind() == NodeKind::BeginNode)
}

/// RuboCop's `ForToEachCorrector`.
fn for_to_each_edit(ctx: &Context<'_>, for_node: &ForNode<'_>) -> Edit {
    let collection = for_node.collection();
    let index = for_node.index();
    let dot: &[u8] = if collection.as_call_node().is_some_and(|c| c.is_safe_navigation()) {
        b"&."
    } else {
        b"."
    };

    let mut replacement = collection_source(ctx, &collection);
    replacement.extend_from_slice(dot);
    replacement.extend_from_slice(b"each do |");
    replacement.extend_from_slice(ctx.text(index.span()));
    replacement.push(b'|');

    let end = if let Some(do_loc) = for_node.do_keyword_loc() {
        do_loc.span().end
    } else {
        collection_end(&collection)
    };
    Edit::replace(Span::new(for_node.for_keyword_loc().span().start, end), replacement)
}

/// RuboCop's `ForToEachCorrector#collection_source`.
fn collection_source(ctx: &Context<'_>, collection: &Node<'_>) -> Vec<u8> {
    if requires_parentheses(collection) {
        let mut out = vec![b'('];
        out.extend_from_slice(ctx.text(collection.span()));
        out.push(b')');
        out
    } else {
        ctx.text(collection.span()).to_vec()
    }
}

/// RuboCop's `ForToEachCorrector#requires_parentheses?`.
fn requires_parentheses(collection: &Node<'_>) -> bool {
    if collection.as_call_node().is_some_and(|c| OPERATOR_METHODS.contains(&c.name().as_slice())) {
        return true;
    }
    matches!(collection.kind(), NodeKind::RangeNode | NodeKind::AndNode | NodeKind::OrNode)
}

/// RuboCop's `ForToEachCorrector#collection_end`: an explicitly parenthesized
/// collection's own range ends at its closing paren; otherwise at the
/// collection expression's own end.
fn collection_end(collection: &Node<'_>) -> u32 {
    collection.as_parentheses_node().map_or(collection.span().end, |p| p.closing_loc().span().end)
}

/// RuboCop's `EachToForCorrector`.
fn each_to_for_edit(
    ctx: &Context<'_>,
    node: &Node<'_>,
    call: &CallNode<'_>,
    block: &BlockNode<'_>,
) -> Edit {
    let collection = call.receiver().expect("checked by caller");
    let collection_source = String::from_utf8_lossy(ctx.text(collection.span())).into_owned();

    let params = block
        .parameters()
        .and_then(|p| p.as_block_parameters_node())
        .filter(|bp| bp.parameters().is_some());

    let (end, correction) = if let Some(params) = &params {
        let inner = params.parameters().expect("filtered Some above");
        let variables = param_texts(ctx, &inner).join(", ");
        (params.location().span().end, format!("for {variables} in {collection_source} do"))
    } else {
        let end = block.opening_loc().span().end;
        (end, format!("for _ in {collection_source} do"))
    };

    Edit::replace(Span::new(node.span().start, end), correction.into_bytes())
}

/// RuboCop's `argument_node.children.map(&:source)`: every top-level
/// parameter's own verbatim source, in declaration order.
fn param_texts(ctx: &Context<'_>, params: &ParametersNode<'_>) -> Vec<String> {
    let mut out = Vec::new();
    let text = |n: &Node<'_>| String::from_utf8_lossy(ctx.text(n.span())).into_owned();
    for n in &params.requireds() {
        out.push(text(&n));
    }
    for n in &params.optionals() {
        out.push(text(&n));
    }
    if let Some(rest) = params.rest() {
        out.push(text(&rest));
    }
    for n in &params.posts() {
        out.push(text(&n));
    }
    for n in &params.keywords() {
        out.push(text(&n));
    }
    if let Some(kwrest) = params.keyword_rest() {
        out.push(text(&kwrest));
    }
    out
}

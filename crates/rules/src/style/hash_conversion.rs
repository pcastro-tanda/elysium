//! `Style/HashConversion`, ported from RuboCop's
//! `lib/rubocop/cop/style/hash_conversion.rb`.
//!
//! Upstream tracks the already-processed outer `Hash[...]` node with
//! `ignore_node`/`part_of_ignored_node?` so a nested `Hash[...]` inside it
//! (e.g. `Hash[Hash[k, v]]`) is never independently re-offended on; `ignored`
//! reimplements that as a span stack, pushed in [`Rule::enter`] and popped in
//! [`Rule::leave`].
//!
//! `add_parentheses` needs the *enclosing* call's own selector end, full
//! span, parenthesized state, and method name (to special-case a `.to_h`
//! receiver) to safely wrap an unparenthesized argument list in `(...)`.
//! Prism gives no parent pointers and [`linter::Context::ancestors`] carries
//! only `(span, kind)` pairs, so [`CallFacts`] records those four details for
//! every `CallNode` as it is entered (always, not just for `Hash[]` ones);
//! [`HashConversion::enclosing_call_facts`] then climbs past the one
//! `ArgumentsNode` wrapper Prism inserts when the matched node is one of
//! several arguments (there is no such wrapper when it is a receiver) to
//! find the enclosing call's own span, and looks up its facts.
//!
//! Only a braceless keyword-style argument (`Hash[a: b]`, Prism's
//! `KeywordHashNode`) is treated as `hash_type?`; an explicit brace literal
//! (`Hash[{a: 1}]`, Prism's `HashNode`) is untested upstream for this cop and
//! would need different wrapping (no extra braces), so it is left to fall
//! through to the generic `.to_h` branch -- see `blind_spots`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext::const_name, node::CallNode, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::collections::HashMap;

const MSG_TO_H: &str = "Prefer `ary.to_h` to `Hash[ary]`.";
const MSG_LITERAL_MULTI_ARG: &str = "Prefer literal hash to `Hash[arg1, arg2, ...]`.";
const MSG_LITERAL_HASH_ARG: &str = "Prefer literal hash to `Hash[key: value, ...]`.";
const MSG_SPLAT: &str = "Prefer `array_of_pairs.to_h` to `Hash[*array]`.";

/// Facts about one `CallNode`, recorded as it is entered so a later
/// `Hash[]` descendant can recover its enclosing call's shape without a
/// parent pointer. See the module doc.
#[derive(Debug, Clone, Copy)]
struct CallFacts {
    /// One byte past the call's own selector -- upstream's
    /// `args_begin`/`loc.selector.end`, the position whose single
    /// character gets replaced by `(`.
    message_end: u32,
    /// Upstream's `parenthesized?`.
    parenthesized: bool,
    /// Upstream's `method?(:to_h)`.
    is_to_h: bool,
    /// Upstream's `args_end`/`node.source_range.end`.
    full_end: u32,
}

/// Upstream's `(send (const {nil? cbase} :Hash) :[] ...)`.
fn is_hash_bracket_call(call: &CallNode<'_>) -> bool {
    !call.is_safe_navigation()
        && call.name().as_slice() == b"[]"
        && call.receiver().is_some_and(|r| const_name(&r).as_deref() == Some("Hash"))
}

/// Upstream's `use_zip_method_without_argument?`.
fn zip_method_without_argument<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() || call.name().as_slice() != b"zip" {
        return None;
    }
    let empty = call.arguments().is_none_or(|a| a.arguments().is_empty());
    empty.then_some(call)
}

/// Upstream's `requires_parens?`.
fn requires_parens(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    if let Some(call) = node.as_call_node() {
        if call.name().as_slice() == b"[]" {
            return false;
        }
        let has_args = call.arguments().is_some_and(|a| !a.arguments().is_empty());
        let parenthesized = call.closing_loc().is_some_and(|l| ctx.text(l.span()) == b")");
        if has_args && !parenthesized {
            return true;
        }
    }
    matches!(node.kind(), NodeKind::AndNode | NodeKind::OrNode)
}

/// Checks the usage of pre-2.1 `Hash[args]` method of converting enumerables and
/// sequences of values to hashes.
#[derive(Debug, Clone)]
pub struct HashConversion {
    /// `AllowSplatArgument`.
    allow_splat_argument: bool,
    /// Upstream's `ignore_node`/`part_of_ignored_node?`: spans of already
    /// processed `Hash[...]` calls, so a nested one is skipped outright.
    ignored: Vec<Span>,
    /// See [`CallFacts`].
    call_facts: HashMap<Span, CallFacts>,
}

impl HashConversion {
    fn part_of_ignored(&self, span: Span) -> bool {
        self.ignored.last().is_some_and(|s| s.start <= span.start && span.end <= s.end)
    }

    /// Climbs past the one `ArgumentsNode` wrapper Prism inserts for an
    /// argument-position match (none exists for a receiver-position match)
    /// to find the enclosing call's own recorded facts.
    fn enclosing_call_facts(&self, ctx: &Context<'_>) -> Option<CallFacts> {
        let ancestors = ctx.ancestors();
        let last = ancestors.last()?;
        let candidate = if last.kind == NodeKind::ArgumentsNode {
            ancestors.get(ancestors.len().checked_sub(2)?)?
        } else {
            last
        };
        (candidate.kind == NodeKind::CallNode)
            .then(|| self.call_facts.get(&candidate.span).copied())
            .flatten()
    }

    /// Upstream's `register_offense_for_hash`.
    fn register_offense_for_hash(&self, ctx: &mut Context<'_>, span: Span, hash_arg: &Node<'_>) {
        let hash_src = String::from_utf8_lossy(ctx.text(hash_arg.span())).into_owned();
        let mut edits = vec![Edit::replace(span, format!("{{{hash_src}}}").into_bytes())];
        if let Some(facts) = self.enclosing_call_facts(ctx) {
            if !facts.parenthesized {
                push_paren_edits(&mut edits, facts);
            }
        }
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG_LITERAL_HASH_ARG,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }

    /// Upstream's `register_offense_for_zip_method`.
    fn register_offense_for_zip_method(ctx: &mut Context<'_>, span: Span, zip_call: &CallNode<'_>) {
        let parenthesized = zip_call.closing_loc().is_some_and(|l| ctx.text(l.span()) == b")");
        let edit = if parenthesized {
            let close = zip_call.closing_loc().expect("checked Some");
            Edit::insert(close.span().start, b"[]".to_vec())
        } else {
            Edit::insert(zip_call.as_node().span().end, b"([])".to_vec())
        };
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG_TO_H,
            Fix { applicability: Applicability::Unsafe, edits: vec![edit] },
        );
    }

    /// Upstream's `single_argument`.
    fn single_argument(&mut self, ctx: &mut Context<'_>, span: Span, first: &Node<'_>) {
        if let Some(kw_hash) = first.as_keyword_hash_node() {
            self.register_offense_for_hash(ctx, span, &kw_hash.as_node());
            return;
        }
        if first.as_splat_node().is_some() {
            if !self.allow_splat_argument {
                ctx.report(&Self::META, span, MSG_SPLAT);
            }
            return;
        }
        if let Some(zip_call) = zip_method_without_argument(first) {
            Self::register_offense_for_zip_method(ctx, span, &zip_call);
            return;
        }
        let source = String::from_utf8_lossy(ctx.text(first.span())).into_owned();
        let replacement = if requires_parens(first, ctx) { format!("({source})") } else { source };
        let edit = Edit::replace(span, format!("{replacement}.to_h").into_bytes());
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG_TO_H,
            Fix { applicability: Applicability::Unsafe, edits: vec![edit] },
        );
    }

    /// Upstream's `multi_argument`/`correct_multi_argument`.
    fn multi_argument(&self, ctx: &mut Context<'_>, span: Span, args: &[Node<'_>]) {
        if args.iter().any(|a| a.as_splat_node().is_some()) {
            return;
        }
        if args.len() % 2 == 1 {
            ctx.report(&Self::META, span, MSG_LITERAL_MULTI_ARG);
            return;
        }
        let content = args
            .chunks(2)
            .map(|pair| {
                let a = String::from_utf8_lossy(ctx.text(pair[0].span()));
                let b = String::from_utf8_lossy(ctx.text(pair[1].span()));
                format!("{a} => {b}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        let mut edits = vec![Edit::replace(span, format!("{{{content}}}").into_bytes())];
        if let Some(facts) = self.enclosing_call_facts(ctx) {
            if !facts.parenthesized && !facts.is_to_h {
                push_paren_edits(&mut edits, facts);
            }
        }
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG_LITERAL_MULTI_ARG,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// Upstream's `add_parentheses`'s else branch: replaces the one character
/// right after the enclosing call's selector with `(`, and appends `)` at
/// its own end.
fn push_paren_edits(edits: &mut Vec<Edit>, facts: CallFacts) {
    edits.push(Edit::replace(Span::new(facts.message_end, facts.message_end + 1), b"(".to_vec()));
    edits.push(Edit::insert(facts.full_end, b")".to_vec()));
}

impl Rule for HashConversion {
    const META: RuleMeta = RuleMeta {
        name: "Style/HashConversion",
        department: Department::Style,
        summary: "Avoid Hash[] in favor of ary.to_h or literal hashes.",
        explanation: "\
Checks the usage of pre-2.1 `Hash[args]` method of converting enumerables and
sequences of values to hashes.

Correction code from splat argument (`Hash[*ary]`) is not simply determined. For example,
`Hash[*ary]` can be replaced with `ary.each_slice(2).to_h` but it will be complicated.
So, `AllowSplatArgument` option is true by default to allow splat argument for simple code.

@safety
This cop's autocorrection is unsafe because `ArgumentError` occurs
if the number of elements is odd:

```ruby
Hash[[[1, 2], [3]]] #=> {1=>2, 3=>nil}
[[1, 2], [5]].to_h  #=> wrong array length at 1 (expected 2, was 1) (ArgumentError)
```

```ruby
# bad
Hash[ary]

# good
ary.to_h

# bad
Hash[key1, value1, key2, value2]

# good
{key1 => value1, key2 => value2}
```

With `AllowSplatArgument: true` (default):

```ruby
# good
Hash[*ary]
```

With `AllowSplatArgument: false`:

```ruby
# bad
Hash[*ary]
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[linter::ConfigOption {
            name: "AllowSplatArgument",
            default: linter::ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Whether `Hash[*ary]` is allowed (its correction would require the more \
                  complex `ary.each_slice(2).to_h`, so it is unsafe to suggest automatically).",
        }],
        blind_spots: "\
Only a braceless keyword-style argument (`Hash[a: b]`, Prism's `KeywordHashNode`) is treated as \
upstream's `hash_type?`; an explicit brace literal (`Hash[{a: 1}]`, Prism's `HashNode`) instead \
falls through to the generic `.to_h` branch, appending `.to_h` to its own verbatim source \
(`{a: 1}.to_h`) rather than splicing its contents into a replacement `{...}` literal.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_splat_argument: options.bool("AllowSplatArgument"),
            ignored: Vec::new(),
            call_facts: HashMap::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let span = node.span();

        self.call_facts.insert(
            span,
            CallFacts {
                message_end: call.message_loc().map_or(span.start, |l| l.span().end),
                parenthesized: call.closing_loc().is_some_and(|l| ctx.text(l.span()) == b")"),
                is_to_h: call.name().as_slice() == b"to_h",
                full_end: span.end,
            },
        );

        if self.part_of_ignored(span) || !is_hash_bracket_call(&call) {
            return;
        }

        let args: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        if args.len() == 1 {
            self.single_argument(ctx, span, &args[0]);
        } else {
            self.multi_argument(ctx, span, &args);
        }
        self.ignored.push(span);
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if self.ignored.last() == Some(&node.span()) {
            self.ignored.pop();
        }
    }
}

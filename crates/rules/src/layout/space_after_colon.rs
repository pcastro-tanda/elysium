//! `Layout/SpaceAfterColon`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_after_colon.rb`.
//!
//! RuboCop dispatches on whitequark's `pair` (a hash/kwargs `key: value`
//! entry) and `kwoptarg` (`def f(a: 1)`'s optional keyword parameter) node
//! types. Prism's equivalents are [`NodeKind::AssocNode`] and
//! [`NodeKind::OptionalKeywordParameterNode`].
//!
//! `on_pair`'s `node.colon?` (`loc.operator.is?(':')`, false for a hash-rocket
//! `"a"=>1` pair) has no direct Prism equivalent: a label-style key (`a: 1`,
//! `"a": 1`, or the shorthand `a:`) always parses with a `nil`
//! [`AssocNode::operator_loc`] and folds the colon into the trailing byte of
//! the key node's own location instead (`SymbolNode` covering `"a:"`), while
//! only the hash-rocket spelling (`"a"=>1`) gives an explicit `operator_loc`
//! (covering `"=>"`, never `":"`). So a `nil` operator with the key's last
//! byte equal to `:` is this cop's colon case; a non-`nil` operator is
//! always the hash-rocket case and is skipped. `node.value_omission?`
//! (`{x:}`'s shorthand value) is Prism's [`NodeKind::ImplicitNode`] value,
//! matching the `value_omission` convention used by
//! `hash_alignment.rs`/`hash_syntax.rs`.
//!
//! `on_kwoptarg` has no direct colon-range field to read, so upstream
//! constructs one from `node.loc.name.end.resize(1)`. Prism's
//! [`OptionalKeywordParameterNode::name_loc`] itself covers the name
//! *plus* the trailing colon (`"var:"`), so the colon is recovered as the
//! one byte just before that location's end instead.
use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Space missing after colon.";

/// True when the byte right after `colon` (a one-byte `:` span) is Ruby
/// whitespace -- RuboCop's `followed_by_space?`
/// (`/\s/.match?(source[end_pos])`).
fn followed_by_space(ctx: &Context<'_>, colon: Span) -> bool {
    ctx.text(Span::new(colon.end, colon.end + 1)).first().is_some_and(u8::is_ascii_whitespace)
}

/// Reports a missing space after `colon`, inserting one -- RuboCop's
/// `register_offense`.
fn register_offense(ctx: &mut Context<'_>, colon: Span) {
    if followed_by_space(ctx, colon) {
        return;
    }
    ctx.report_with_fix(
        &SpaceAfterColon::META,
        colon,
        MSG,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::insert(colon.end, b" ".to_vec())],
        },
    );
}

/// Use spaces after colons.
#[derive(Debug, Clone)]
pub struct SpaceAfterColon;

impl Rule for SpaceAfterColon {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceAfterColon",
        department: Department::Layout,
        summary: "Use spaces after colons.",
        explanation: "Checks for colon (`:`) not followed by some kind of space.\n\
N.B. this cop does not handle spaces after a ternary operator, which are\n\
instead handled by `Layout/SpaceAroundOperators`.\n\n\
```ruby\n# bad\ndef f(a:, b:2); {a:3}; end\n\n# good\ndef f(a:, b: 2); {a: 3}; end\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::AssocNode, NodeKind::OptionalKeywordParameterNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::AssocNode { .. } => {
                let assoc = node.as_assoc_node().expect("kind matched");
                // Hash-rocket pair (`"a"=>1`): never a colon.
                if assoc.operator_loc().is_some() {
                    return;
                }
                // Label-form pair (`a: 1`, `"a": 1`, shorthand `a:`): the
                // colon is the key's own trailing byte.
                let key_span = assoc.key().span();
                let Some(colon_start) = key_span.end.checked_sub(1) else { return };
                let colon_span = Span::new(colon_start, key_span.end);
                if ctx.text(colon_span) != b":" {
                    return;
                }
                if assoc.value().as_implicit_node().is_some() {
                    return;
                }
                register_offense(ctx, colon_span);
            }
            Node::OptionalKeywordParameterNode { .. } => {
                let kwoptarg = node.as_optional_keyword_parameter_node().expect("kind matched");
                let name_end = kwoptarg.name_loc().span().end;
                let Some(colon_start) = name_end.checked_sub(1) else { return };
                register_offense(ctx, Span::new(colon_start, name_end));
            }
            _ => {}
        }
    }
}

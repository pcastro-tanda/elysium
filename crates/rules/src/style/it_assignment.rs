//! `Style/ItAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/style/it_assignment.rb`.
//!
//! whitequark's `lvasgn`/`arg`/`optarg`/`restarg`/`blockarg`/`kwarg`/
//! `kwoptarg`/`kwrestarg` map onto Prism's `LocalVariableWriteNode`,
//! `RequiredParameterNode`, `OptionalParameterNode`, `RestParameterNode`,
//! `BlockParameterNode`, `RequiredKeywordParameterNode`,
//! `OptionalKeywordParameterNode` and `KeywordRestParameterNode`
//! respectively. `RequiredParameterNode` has no `name_loc` of its own (its
//! whole span *is* the name), so its own span is used directly; every other
//! kind reports at its `name_loc` (an anonymous `*it`/`**it`/`&it` still has
//! a `name_loc`, since the check only fires when a name was actually given).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "`it` is the default block parameter; consider another name.";

/// Checks for local variables and method parameters named `it`.
#[derive(Debug, Clone)]
pub struct ItAssignment;

impl Rule for ItAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Style/ItAssignment",
        department: Department::Style,
        summary: "Checks for local variables and method parameters named `it`.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::LocalVariableWriteNode,
            NodeKind::RequiredParameterNode,
            NodeKind::OptionalParameterNode,
            NodeKind::RestParameterNode,
            NodeKind::BlockParameterNode,
            NodeKind::RequiredKeywordParameterNode,
            NodeKind::OptionalKeywordParameterNode,
            NodeKind::KeywordRestParameterNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(lvasgn) = node.as_local_variable_write_node() {
            if lvasgn.name().as_slice() == b"it" {
                ctx.report(&Self::META, lvasgn.name_loc().span(), MSG);
            }
        } else if let Some(arg) = node.as_required_parameter_node() {
            if arg.name().as_slice() == b"it" {
                ctx.report(&Self::META, node.span(), MSG);
            }
        } else if let Some(optarg) = node.as_optional_parameter_node() {
            if optarg.name().as_slice() == b"it" {
                ctx.report(&Self::META, optarg.name_loc().span(), MSG);
            }
        } else if let Some(restarg) = node.as_rest_parameter_node() {
            if restarg.name().is_some_and(|name| name.as_slice() == b"it") {
                if let Some(name_loc) = restarg.name_loc() {
                    ctx.report(&Self::META, name_loc.span(), MSG);
                }
            }
        } else if let Some(blockarg) = node.as_block_parameter_node() {
            if blockarg.name().is_some_and(|name| name.as_slice() == b"it") {
                if let Some(name_loc) = blockarg.name_loc() {
                    ctx.report(&Self::META, name_loc.span(), MSG);
                }
            }
        } else if let Some(kwarg) = node.as_required_keyword_parameter_node() {
            if kwarg.name().as_slice() == b"it" {
                ctx.report(&Self::META, name_span(&kwarg.name_loc()), MSG);
            }
        } else if let Some(kwoptarg) = node.as_optional_keyword_parameter_node() {
            if kwoptarg.name().as_slice() == b"it" {
                ctx.report(&Self::META, name_span(&kwoptarg.name_loc()), MSG);
            }
        } else if let Some(kwrestarg) = node.as_keyword_rest_parameter_node() {
            if kwrestarg.name().is_some_and(|name| name.as_slice() == b"it") {
                if let Some(name_loc) = kwrestarg.name_loc() {
                    ctx.report(&Self::META, name_loc.span(), MSG);
                }
            }
        }
    }
}

/// `RequiredKeywordParameterNode`/`OptionalKeywordParameterNode`'s
/// `name_loc` spans the whole `name:` token including the trailing colon;
/// upstream's `node.loc.name` is just the name itself.
fn name_span(name_loc: &ruby_ast::Location<'_>) -> Span {
    let span = name_loc.span();
    Span::new(span.start, span.end - 1)
}

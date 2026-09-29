//! `Lint/UnderscorePrefixedVariableName`, ported from RuboCop's
//! `lib/rubocop/cop/lint/underscore_prefixed_variable_name.rb`.
//!
//! RuboCop hooks `after_leaving_scope` via `VariableForce`; here every
//! variable in the file ([`ruby_semantic::Semantics::variable_ids`]) is
//! checked in a single pass on `ProgramNode`. `node.match_with_lvasgn_type?`
//! (RuboCop's whitequark check for `/re/ =~ str` named captures) is
//! [`ruby_semantic::DeclKind::RegexpNamedCapture`]; its offense location is
//! the regexp node (the match's receiver), matching upstream's
//! `node.children.first.source_range`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_semantic::{DeclKind, Semantics, VariableId};
use ruby_source::Span;

const MSG: &str = "Do not use prefix `_` for a variable that is used.";

/// `Lint::UnderscorePrefixedVariableName`.
#[derive(Debug, Clone)]
pub struct UnderscorePrefixedVariableName {
    allow_keyword_block_arguments: bool,
}

impl Rule for UnderscorePrefixedVariableName {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UnderscorePrefixedVariableName",
        department: Department::Lint,
        summary: "Do not use prefix `_` for a variable that is used.",
        explanation: "\
Checks for underscore-prefixed variables that are actually
used.

Since block keyword arguments cannot be arbitrarily named at call
sites, the `AllowKeywordBlockArguments` will allow use of underscore-
prefixed block keyword arguments.

```ruby
# bad
[1, 2, 3].each do |_num|
  do_something(_num)
end

query(:sales) do |_id:, revenue:, cost:|
  {_id: _id, profit: revenue - cost}
end

# good
[1, 2, 3].each do |num|
  do_something(num)
end

[1, 2, 3].each do |_num|
  do_something # not using `_num`
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ProgramNode],
        config: &[linter::ConfigOption {
            name: "AllowKeywordBlockArguments",
            default: linter::ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Allows use of underscore-prefixed block keyword arguments.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_keyword_block_arguments: options.bool("AllowKeywordBlockArguments") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if node.kind() != NodeKind::ProgramNode {
            return;
        }
        let mut offenses: Vec<Span> = Vec::new();
        {
            let semantics = ctx.semantics();
            for id in semantics.variable_ids() {
                if let Some(span) = self.check_variable(semantics, id) {
                    offenses.push(span);
                }
            }
        }
        for span in offenses {
            ctx.report(&Self::META, span, MSG);
        }
    }
}

impl UnderscorePrefixedVariableName {
    /// RuboCop's `check_variable`.
    fn check_variable(&self, semantics: &Semantics<'_>, id: VariableId) -> Option<Span> {
        let variable = semantics.variable(id);
        if !variable.should_be_unused() {
            return None;
        }
        if !variable.references().iter().any(ruby_semantic::Reference::explicit) {
            return None;
        }
        if self.allowed_keyword_block_argument(semantics, id) {
            return None;
        }

        let node = variable.declaration();
        Some(match variable.decl_kind() {
            DeclKind::RegexpNamedCapture => {
                let match_write = node.as_match_write_node().expect("RegexpNamedCapture node");
                match_write.call().receiver().map_or_else(|| node.span(), |regexp| regexp.span())
            }
            DeclKind::Assignment => {
                let write = node.as_local_variable_write_node().expect("Assignment node");
                write.name_loc().span()
            }
            DeclKind::OptionalArg => {
                let arg = node.as_optional_parameter_node().expect("OptionalArg node");
                arg.name_loc().span()
            }
            DeclKind::KeywordArg => {
                let arg = node.as_required_keyword_parameter_node().expect("KeywordArg node");
                name_span_without_colon(arg.name_loc().span(), variable.name())
            }
            DeclKind::OptionalKeywordArg => {
                let arg =
                    node.as_optional_keyword_parameter_node().expect("OptionalKeywordArg node");
                name_span_without_colon(arg.name_loc().span(), variable.name())
            }
            DeclKind::RestArg => {
                let arg = node.as_rest_parameter_node().expect("RestArg node");
                arg.name_loc().map_or_else(|| node.span(), |loc| loc.span())
            }
            DeclKind::KeywordRestArg => {
                let arg = node.as_keyword_rest_parameter_node().expect("KeywordRestArg node");
                arg.name_loc().map_or_else(|| node.span(), |loc| loc.span())
            }
            DeclKind::BlockArg => {
                let arg = node.as_block_parameter_node().expect("BlockArg node");
                arg.name_loc().map_or_else(|| node.span(), |loc| loc.span())
            }
            DeclKind::RequiredArg | DeclKind::BlockLocal | DeclKind::PatternMatch => node.span(),
        })
    }

    /// RuboCop's `allowed_keyword_block_argument?`.
    fn allowed_keyword_block_argument(&self, semantics: &Semantics<'_>, id: VariableId) -> bool {
        semantics.is_block_argument(id)
            && semantics.variable(id).is_keyword_argument()
            && self.allow_keyword_block_arguments
    }
}

/// Prism's `name_loc` for keyword parameters spans the trailing `:` as
/// well (`_foo:`); upstream's `node.loc.name` does not, so the span is
/// clipped back to the bare name's byte length.
fn name_span_without_colon(name_loc: Span, name: &[u8]) -> Span {
    let len = u32::try_from(name.len()).expect("name length exceeds u32");
    Span::new(name_loc.start, name_loc.start + len)
}

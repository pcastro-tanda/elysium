//! `Naming/VariableName`, ported from RuboCop's
//! `lib/rubocop/cop/naming/variable_name.rb`, sharing the
//! `ConfigurableNaming` style regexes and identifier/pattern lists with
//! [`super::method_name`] via [`super::configurable_naming`].
//!
//! # Node mapping
//!
//! Upstream subscribes to every whitequark assignment/parameter node
//! (`lvasgn`, `ivasgn`, `cvasgn`, `arg`, `optarg`, `restarg`, `kwoptarg`,
//! `kwarg`, `kwrestarg`, `blockarg`, `lvar`) plus `gvasgn` for the forbidden
//! check only. Prism splits those into more kinds: an `op_asgn`/`or_asgn`/
//! `and_asgn` whose target is an `lvasgn` becomes its own
//! `*OperatorWriteNode`/`*OrWriteNode`/`*AndWriteNode`, and the `lvasgn`
//! targets of multiple assignment, `rescue => e` and `for x in` become
//! `LocalVariableTargetNode`. All of them carry the variable's name and are
//! visited here.
//!
//! `LocalVariableTargetNode` is also what Prism produces for a
//! pattern-matching binding (`in [a, b]`, `in x`, `=> n`), which whitequark
//! represents as `match_var` -- a node upstream never subscribes to. Targets
//! whose nearest enclosing statement-or-pattern ancestor is a pattern are
//! therefore skipped ([`in_pattern`]).

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::configurable_naming::{identifier_matches, matches_style, pattern_matches, Style};

/// Makes sure that all variables use the configured style, `snake_case` or
/// camelCase, for their names.
#[derive(Debug, Clone)]
pub struct VariableName {
    style: Style,
    allowed_identifiers: Vec<String>,
    allowed_patterns: Vec<Regex>,
    forbidden_identifiers: Vec<String>,
    forbidden_patterns: Vec<Regex>,
}

const KINDS: &[NodeKind] = &[
    NodeKind::LocalVariableWriteNode,
    NodeKind::LocalVariableOrWriteNode,
    NodeKind::LocalVariableAndWriteNode,
    NodeKind::LocalVariableOperatorWriteNode,
    NodeKind::LocalVariableTargetNode,
    NodeKind::LocalVariableReadNode,
    NodeKind::InstanceVariableWriteNode,
    NodeKind::InstanceVariableOrWriteNode,
    NodeKind::InstanceVariableAndWriteNode,
    NodeKind::InstanceVariableOperatorWriteNode,
    NodeKind::InstanceVariableTargetNode,
    NodeKind::ClassVariableWriteNode,
    NodeKind::ClassVariableOrWriteNode,
    NodeKind::ClassVariableAndWriteNode,
    NodeKind::ClassVariableOperatorWriteNode,
    NodeKind::ClassVariableTargetNode,
    NodeKind::GlobalVariableWriteNode,
    NodeKind::GlobalVariableOrWriteNode,
    NodeKind::GlobalVariableAndWriteNode,
    NodeKind::GlobalVariableOperatorWriteNode,
    NodeKind::GlobalVariableTargetNode,
    NodeKind::RequiredParameterNode,
    NodeKind::OptionalParameterNode,
    NodeKind::RestParameterNode,
    NodeKind::OptionalKeywordParameterNode,
    NodeKind::RequiredKeywordParameterNode,
    NodeKind::KeywordRestParameterNode,
    NodeKind::BlockParameterNode,
];

impl Rule for VariableName {
    const META: RuleMeta = RuleMeta {
        name: "Naming/VariableName",
        department: Department::Naming,
        summary: "Makes sure that all variables use the configured style, snake_case or camelCase, for their names.",
        explanation: "\
```ruby
# EnforcedStyle: snake_case (default)

# bad
fooBar = 1

# good
foo_bar = 1
```

```ruby
# EnforcedStyle: camelCase

# bad
foo_bar = 1

# good
fooBar = 1
```

```ruby
# AllowedIdentifiers: ['fooBar']

# good (with EnforcedStyle: snake_case)
fooBar = 1
```

```ruby
# AllowedPatterns: ['_v\\d+\\z']

# good (with EnforcedStyle: camelCase)
release_v1 = true
```

```ruby
# ForbiddenIdentifiers: ['fooBar']

# bad
fooBar = 1
```

```ruby
# ForbiddenPatterns: ['_v\\d+\\z']

# bad
release_v1 = true
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: KINDS,
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("snake_case"),
                allowed: &["snake_case", "camelCase"],
                doc: "Naming style variables must follow.",
            },
            ConfigOption {
                name: "AllowedIdentifiers",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Variable names (sigils stripped) that are never checked.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Regexps; a variable name matching one is accepted regardless of style.",
            },
            ConfigOption {
                name: "ForbiddenIdentifiers",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Variable names (sigils stripped) that are always flagged.",
            },
            ConfigOption {
                name: "ForbiddenPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Regexps; a variable name matching one is always flagged.",
            },
        ],
        blind_spots: "`it` and numbered block parameters (`_1`) are read through dedicated Prism \
nodes and are never checked; both always satisfy `snake_case`.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            style: Style::parse(options.style("EnforcedStyle")?),
            allowed_identifiers: options.str_list("AllowedIdentifiers"),
            allowed_patterns: compile(&options.str_list("AllowedPatterns")),
            forbidden_identifiers: options.str_list("ForbiddenIdentifiers"),
            forbidden_patterns: compile(&options.str_list("ForbiddenPatterns")),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some((name, span)) = name_and_span(node) else { return };
        if node.kind() == NodeKind::LocalVariableTargetNode && in_pattern(ctx) {
            return;
        }
        if is_global(node.kind()) {
            // Upstream `on_gvasgn`: forbidden names only, never the style.
            if self.forbidden(name) {
                Self::report_forbidden(ctx, span, name);
            }
            return;
        }
        if identifier_matches(&self.allowed_identifiers, name) {
            return;
        }
        if self.forbidden(name) {
            Self::report_forbidden(ctx, span, name);
        } else if !matches_style(self.style, name) && !pattern_matches(&self.allowed_patterns, name)
        {
            ctx.report(
                &Self::META,
                span,
                format!("Use {} for variable names.", self.style.config_name()),
            );
        }
    }
}

impl VariableName {
    fn forbidden(&self, name: &[u8]) -> bool {
        identifier_matches(&self.forbidden_identifiers, name)
            || pattern_matches(&self.forbidden_patterns, name)
    }

    fn report_forbidden(ctx: &mut Context<'_>, span: Span, name: &[u8]) {
        ctx.report(
            &Self::META,
            span,
            format!("`{}` is forbidden, use another name instead.", String::from_utf8_lossy(name)),
        );
    }
}

fn compile(patterns: &[String]) -> Vec<Regex> {
    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
}

const fn is_global(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::GlobalVariableWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::GlobalVariableTargetNode
    )
}

/// Whether a `LocalVariableTargetNode` is a pattern-matching binding
/// (whitequark `match_var`, which upstream never visits) rather than a
/// multiple-assignment / `rescue` / `for` target. Walks ancestors innermost
/// first: a pattern kind before any statement list means pattern.
fn in_pattern(ctx: &Context<'_>) -> bool {
    for ancestor in ctx.ancestors().iter().rev() {
        match ancestor.kind {
            NodeKind::ArrayPatternNode
            | NodeKind::HashPatternNode
            | NodeKind::FindPatternNode
            | NodeKind::CapturePatternNode
            | NodeKind::AlternationPatternNode
            | NodeKind::InNode
            | NodeKind::MatchPredicateNode
            | NodeKind::MatchRequiredNode => return true,
            NodeKind::StatementsNode
            | NodeKind::MultiWriteNode
            | NodeKind::MultiTargetNode
            | NodeKind::RescueNode
            | NodeKind::ForNode => return false,
            _ => {}
        }
    }
    false
}

/// The variable's name (sigil included, as upstream's `node.name`) and the
/// range upstream reports on (`node.loc.name`; the whole node for `arg`s and
/// `lvar`s, whose whitequark location has no separate name part).
fn name_and_span<'pr>(node: &Node<'pr>) -> Option<(&'pr [u8], Span)> {
    macro_rules! name_loc {
        ($n:expr) => {{
            let n = $n;
            Some((n.name().as_slice(), n.name_loc().span()))
        }};
    }
    macro_rules! opt_name_loc {
        ($n:expr) => {{
            let n = $n;
            Some((n.name()?.as_slice(), n.name_loc()?.span()))
        }};
    }
    match node.kind() {
        NodeKind::LocalVariableWriteNode => name_loc!(node.as_local_variable_write_node()?),
        NodeKind::LocalVariableOrWriteNode => name_loc!(node.as_local_variable_or_write_node()?),
        NodeKind::LocalVariableAndWriteNode => name_loc!(node.as_local_variable_and_write_node()?),
        NodeKind::LocalVariableOperatorWriteNode => {
            name_loc!(node.as_local_variable_operator_write_node()?)
        }
        NodeKind::LocalVariableTargetNode => {
            Some((node.as_local_variable_target_node()?.name().as_slice(), node.span()))
        }
        NodeKind::LocalVariableReadNode => {
            Some((node.as_local_variable_read_node()?.name().as_slice(), node.span()))
        }
        NodeKind::InstanceVariableWriteNode => name_loc!(node.as_instance_variable_write_node()?),
        NodeKind::InstanceVariableOrWriteNode => {
            name_loc!(node.as_instance_variable_or_write_node()?)
        }
        NodeKind::InstanceVariableAndWriteNode => {
            name_loc!(node.as_instance_variable_and_write_node()?)
        }
        NodeKind::InstanceVariableOperatorWriteNode => {
            name_loc!(node.as_instance_variable_operator_write_node()?)
        }
        NodeKind::InstanceVariableTargetNode => {
            Some((node.as_instance_variable_target_node()?.name().as_slice(), node.span()))
        }
        NodeKind::ClassVariableWriteNode => name_loc!(node.as_class_variable_write_node()?),
        NodeKind::ClassVariableOrWriteNode => name_loc!(node.as_class_variable_or_write_node()?),
        NodeKind::ClassVariableAndWriteNode => name_loc!(node.as_class_variable_and_write_node()?),
        NodeKind::ClassVariableOperatorWriteNode => {
            name_loc!(node.as_class_variable_operator_write_node()?)
        }
        NodeKind::ClassVariableTargetNode => {
            Some((node.as_class_variable_target_node()?.name().as_slice(), node.span()))
        }
        NodeKind::GlobalVariableWriteNode => name_loc!(node.as_global_variable_write_node()?),
        NodeKind::GlobalVariableOrWriteNode => name_loc!(node.as_global_variable_or_write_node()?),
        NodeKind::GlobalVariableAndWriteNode => {
            name_loc!(node.as_global_variable_and_write_node()?)
        }
        NodeKind::GlobalVariableOperatorWriteNode => {
            name_loc!(node.as_global_variable_operator_write_node()?)
        }
        NodeKind::GlobalVariableTargetNode => {
            Some((node.as_global_variable_target_node()?.name().as_slice(), node.span()))
        }
        NodeKind::RequiredParameterNode => {
            Some((node.as_required_parameter_node()?.name().as_slice(), node.span()))
        }
        NodeKind::OptionalParameterNode => name_loc!(node.as_optional_parameter_node()?),
        NodeKind::RestParameterNode => opt_name_loc!(node.as_rest_parameter_node()?),
        NodeKind::KeywordRestParameterNode => opt_name_loc!(node.as_keyword_rest_parameter_node()?),
        NodeKind::BlockParameterNode => opt_name_loc!(node.as_block_parameter_node()?),
        // Prism's keyword `name_loc` covers the trailing `:`; upstream's
        // `loc.name` does not.
        NodeKind::OptionalKeywordParameterNode => {
            let n = node.as_optional_keyword_parameter_node()?;
            Some((n.name().as_slice(), keyword_span(n.name().as_slice(), n.name_loc().span())))
        }
        NodeKind::RequiredKeywordParameterNode => {
            let n = node.as_required_keyword_parameter_node()?;
            Some((n.name().as_slice(), keyword_span(n.name().as_slice(), n.name_loc().span())))
        }
        _ => None,
    }
}

fn keyword_span(name: &[u8], loc: Span) -> Span {
    let len = u32::try_from(name.len()).unwrap_or(loc.len());
    Span::new(loc.start, loc.start.saturating_add(len).min(loc.end))
}

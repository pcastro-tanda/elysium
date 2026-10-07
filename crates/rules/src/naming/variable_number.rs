//! `Naming/VariableNumber`, ported from RuboCop's
//! `lib/rubocop/cop/naming/variable_number.rb`, which mixes in
//! `AllowedIdentifiers`, `ConfigurableNumbering` (style regexes) and
//! `AllowedPattern`.
//!
//! # Node mapping
//!
//! Upstream only subscribes to `on_arg` (aliased to `on_lvasgn`,
//! `on_ivasgn`, `on_cvasgn`, `on_gvasgn`), `on_def`/`on_defs` and `on_sym` --
//! notably *not* `on_optarg`/`on_restarg`/`on_kwarg`/`on_kwoptarg`/
//! `on_kwrestarg`/`on_blockarg`/`on_lvar`, so optional/rest/keyword/block
//! parameters and variable reads are never checked, only required
//! positional parameters (`RequiredParameterNode`) and assignments. Prism
//! splits `lvasgn`/`ivasgn`/`cvasgn`/`gvasgn` into a `*WriteNode` for a
//! plain assignment, a `*OperatorWriteNode`/`*OrWriteNode`/`*AndWriteNode`
//! for `+=`/`||=`/`&&=`, and a `*TargetNode` for multiple assignment,
//! `rescue => e` and `for x in` targets; all are visited here. A
//! `LocalVariableTargetNode` is also what Prism produces for a
//! pattern-matching binding (whitequark's `match_var`, which upstream never
//! subscribes to), so those are skipped ([`in_pattern`]).
//!
//! # Hash-key symbols
//!
//! `on_sym` reports the whole `sym` node, but whitequark's `pair_keyword`/
//! `pair_quoted` builders give a label key (`foo_1: v`, `"foo_1": v`) a
//! range that stops *before* the trailing `:`, which Prism's `SymbolNode`
//! includes in its `closing_loc` (`:` / `":`). [`symbol_span`] drops it.
//!
//! # `class_emitter_method?` blind spot
//!
//! `ConfigurableFormatting#valid_name?` (which `VariableNumber` calls via
//! `super`) also accepts a singleton method whose name matches a sibling
//! class/module defined in the same body (e.g. `def self.Foo1; end` next to
//! `class Foo1; end`) regardless of style. That check requires walking back
//! up to the enclosing body to scan sibling `class` nodes purely for this
//! rare metaprogramming pattern; it is not implemented here and is not
//! exercised by any upstream spec for this cop.

use std::sync::LazyLock;

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::SymbolNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `ConfigurableNumbering::FORMATS[:snake_case]`.
static SNAKE_CASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\D|_\d+|\A\d+)\z").expect("valid regex"));

/// RuboCop's `ConfigurableNumbering::FORMATS[:normalcase]`, with the
/// interpolated `implicit_param = /\A_\d+\z/` inlined as a second
/// alternative.
static NORMAL_CASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\D|[^_\d]\d+|\A\d+)\z|\A_\d+\z").expect("valid regex"));

/// RuboCop's `ConfigurableNumbering::FORMATS[:non_integer]`, with
/// `implicit_param` inlined the same way.
static NON_INTEGER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\D|\A\d+)\z|\A_\d+\z").expect("valid regex"));

/// RuboCop's `EnforcedStyle` for this cop (`SupportedStyles` in
/// `default.yml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    SnakeCase,
    NormalCase,
    NonInteger,
}

impl Style {
    fn parse(s: &str) -> Self {
        match s {
            "snake_case" => Self::SnakeCase,
            "non_integer" => Self::NonInteger,
            _ => Self::NormalCase,
        }
    }

    /// The style's config name, used verbatim in the message (upstream's
    /// `style.to_s`, e.g. `:normalcase.to_s == "normalcase"`).
    const fn config_name(self) -> &'static str {
        match self {
            Self::SnakeCase => "snake_case",
            Self::NormalCase => "normalcase",
            Self::NonInteger => "non_integer",
        }
    }

    fn format(self) -> &'static Regex {
        match self {
            Self::SnakeCase => &SNAKE_CASE,
            Self::NormalCase => &NORMAL_CASE,
            Self::NonInteger => &NON_INTEGER,
        }
    }
}

/// Use the configured style when numbering symbols, methods and variables.
#[derive(Debug, Clone)]
pub struct VariableNumber {
    style: Style,
    check_method_names: bool,
    check_symbols: bool,
    allowed_identifiers: Vec<String>,
    allowed_patterns: Vec<Regex>,
}

const KINDS: &[NodeKind] = &[
    NodeKind::LocalVariableWriteNode,
    NodeKind::LocalVariableOrWriteNode,
    NodeKind::LocalVariableAndWriteNode,
    NodeKind::LocalVariableOperatorWriteNode,
    NodeKind::LocalVariableTargetNode,
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
    NodeKind::DefNode,
    NodeKind::SymbolNode,
];

impl Rule for VariableNumber {
    const META: RuleMeta = RuleMeta {
        name: "Naming/VariableNumber",
        department: Department::Naming,
        summary: "Use the configured style when numbering symbols, methods and variables.",
        explanation: "\
```ruby
# EnforcedStyle: normalcase (default)

# bad
:some_sym_1
variable_1 = 1

def some_method_1; end

def some_method1(arg_1); end

# good
:some_sym1
variable1 = 1

def some_method1; end

def some_method1(arg1); end
```

```ruby
# EnforcedStyle: snake_case

# bad
:some_sym1
variable1 = 1

def some_method1; end

def some_method_1(arg1); end

# good
:some_sym_1
variable_1 = 1

def some_method_1; end

def some_method_1(arg_1); end
```

```ruby
# EnforcedStyle: non_integer

# bad
:some_sym1
:some_sym_1

variable1 = 1
variable_1 = 1

def some_method1; end

def some_method_1; end

def some_methodone(arg1); end
def some_methodone(arg_1); end

# good
:some_symone
:some_sym_one

variableone = 1
variable_one = 1

def some_methodone; end

def some_method_one; end

def some_methodone(argone); end
def some_methodone(arg_one); end
```

```ruby
# AllowedIdentifiers: [capture3]

# good
expect(Open3).to receive(:capture3)
```

```ruby
# AllowedPatterns: ['_v\\d+\\z']

# good
:some_sym_v1
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: KINDS,
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("normalcase"),
                allowed: &["snake_case", "normalcase", "non_integer"],
                doc: "Naming style numbered symbols, methods and variables must follow.",
            },
            ConfigOption {
                name: "CheckMethodNames",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether method names are checked.",
            },
            ConfigOption {
                name: "CheckSymbols",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether symbols are checked.",
            },
            ConfigOption {
                name: "AllowedIdentifiers",
                default: ConfigDefault::StrList(&[
                    "TLS1_1",
                    "TLS1_2",
                    "capture3",
                    "iso8601",
                    "rfc1123_date",
                    "rfc822",
                    "rfc2822",
                    "rfc3339",
                    "x86_64",
                ]),
                allowed: &[],
                doc: "Identifiers (sigils stripped) that are never checked.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Regexps; an identifier matching one is accepted regardless of style.",
            },
        ],
        blind_spots: "The `class_emitter_method?` escape hatch (a singleton method named after \
a sibling class, e.g. `def self.Foo1; end` next to `class Foo1; end`) is not implemented; such a \
method is flagged even though upstream would accept it.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            style: Style::parse(options.style("EnforcedStyle")?),
            check_method_names: options.bool("CheckMethodNames"),
            check_symbols: options.bool("CheckSymbols"),
            allowed_identifiers: options.str_list("AllowedIdentifiers"),
            allowed_patterns: compile_patterns(&options.str_list("AllowedPatterns")),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode => {
                if !self.check_method_names {
                    return;
                }
                let Some(def) = node.as_def_node() else { return };
                let name = def.name().as_slice();
                if identifier_matches(&self.allowed_identifiers, name) {
                    return;
                }
                self.check(ctx, name, def.name_loc().span(), "method name");
            }
            NodeKind::SymbolNode => {
                if !self.check_symbols {
                    return;
                }
                let Some(sym) = node.as_symbol_node() else { return };
                let name = sym.unescaped();
                // Prism parses a quoted empty hash key (`{ "": value }`) as
                // an empty symbol node.
                if name.is_empty() || identifier_matches(&self.allowed_identifiers, name) {
                    return;
                }
                self.check(ctx, name, symbol_span(&sym), "symbol");
            }
            NodeKind::LocalVariableTargetNode if in_pattern(ctx) => {}
            _ => {
                let Some((name, span)) = name_and_span(node) else { return };
                if identifier_matches(&self.allowed_identifiers, name) {
                    return;
                }
                self.check(ctx, name, span, "variable");
            }
        }
    }
}

impl VariableNumber {
    /// RuboCop's `ConfigurableFormatting#check_name`, minus the
    /// `class_emitter_method?` escape hatch (see the module doc).
    fn check(&self, ctx: &mut Context<'_>, name: &[u8], span: Span, identifier_type: &str) {
        if matches_style(self.style, name) || pattern_matches(&self.allowed_patterns, name) {
            return;
        }
        ctx.report(
            &Self::META,
            span,
            format!("Use {} for {identifier_type} numbers.", self.style.config_name()),
        );
    }
}

/// RuboCop's `ConfigurableFormatting#valid_name?`, minus the
/// `class_emitter_method?` escape hatch.
fn matches_style(style: Style, name: &[u8]) -> bool {
    match std::str::from_utf8(name) {
        Ok(s) => style.format().is_match(s),
        Err(_) => false,
    }
}

/// `AllowedIdentifiers`' `SIGILS` constant: `@`/`@@`/`$` are stripped before
/// an identifier is compared, so `AllowedIdentifiers: [fooBar]` also allows
/// `@fooBar`/`@@fooBar`/`$fooBar`. A sigil only ever leads an identifier, so
/// a byte slice suffices -- no per-comparison `Vec` allocation.
fn strip_sigils(name: &[u8]) -> &[u8] {
    match name {
        [b'@', b'@', rest @ ..] | [b'@' | b'$', rest @ ..] => rest,
        _ => name,
    }
}

/// RuboCop's `AllowedIdentifiers#allowed_identifier?`: `name`, sigils
/// stripped, exactly matches one entry of `list`.
fn identifier_matches(list: &[String], name: &[u8]) -> bool {
    if list.is_empty() {
        return false;
    }
    let stripped = strip_sigils(name);
    list.iter().any(|candidate| candidate.as_bytes() == stripped)
}

/// RuboCop's `AllowedPattern#matches_allowed_pattern?`: `name` (sigils
/// intact) matches any of `patterns`.
fn pattern_matches(patterns: &[Regex], name: &[u8]) -> bool {
    if patterns.is_empty() {
        return false;
    }
    match std::str::from_utf8(name) {
        Ok(s) => patterns.iter().any(|pattern| pattern.is_match(s)),
        Err(_) => false,
    }
}

/// Compiles the `AllowedPatterns` config list into regexes, silently
/// dropping any entry that fails to compile.
fn compile_patterns(patterns: &[String]) -> Vec<Regex> {
    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
}

/// The whitequark `sym` node range: Prism's span, minus a label's trailing
/// `:` (see the module doc). A `%s:foo:` literal's closing `:` is its
/// delimiter, not a label colon, so a `%`-opened symbol keeps its span.
fn symbol_span(sym: &SymbolNode<'_>) -> Span {
    let span = sym.as_node().span();
    let is_label = sym.closing_loc().is_some_and(|c| c.as_slice().ends_with(b":"))
        && sym.opening_loc().is_none_or(|o| !o.as_slice().starts_with(b"%"));
    if is_label {
        Span::new(span.start, span.end - 1)
    } else {
        span
    }
}

/// The variable's name (sigil included, as upstream's `node.name`) and the
/// range upstream reports on (`node.loc.name`; the whole node for the
/// `arg`/`*TargetNode` shapes, whose whitequark location has no separate
/// name part).
fn name_and_span<'pr>(node: &Node<'pr>) -> Option<(&'pr [u8], Span)> {
    macro_rules! name_loc {
        ($n:expr) => {{
            let n = $n;
            Some((n.name().as_slice(), n.name_loc().span()))
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
        _ => None,
    }
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

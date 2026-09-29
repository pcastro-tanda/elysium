//! `Naming/PredicatePrefix`, ported from RuboCop's
//! `lib/rubocop/cop/naming/predicate_prefix.rb`.
//!
//! # Scope
//!
//! Two upstream callbacks are ported:
//!
//! - `on_send`: `define_method(:name)`/`def_node_matcher(:name, ...)`-style
//!   dynamic method definitions (a bare call to one of `MethodDefinitionMacros`
//!   whose first argument is a symbol literal). The offense span is the symbol
//!   literal itself (including the leading `:`, matching whitequark's `sym`
//!   node location).
//! - `on_def`/`on_defs`: ordinary method definitions. The offense span is the
//!   method name (`DefNode::name_loc`).
//!
//! `UseSorbetSigs` needs the preceding sibling statement (upstream's
//! `node.left_sibling`) to be a Sorbet `sig { ... }`/`sig do ... end` block
//! whose body is a single `returns(T::Boolean)`-shaped call (`.returns` may be
//! chained onto anything, e.g. `params(...).returns(T::Boolean)`). Prism has
//! no `left_sibling`, so `file_start` walks the tree once, and for every
//! `StatementsNode` records which immediate children are directly preceded by
//! such a `sig`.
//!
//! Not ported: `AllCops/UseProjectIndex` cross-file override detection
//! (`overrides_inherited_method?`), which needs the `rubydex` gem's project
//! index -- no such index exists in elysium, and no fixture exercises it.

use std::collections::HashSet;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `Naming/PredicatePrefix`: predicate method names must not be prefixed
/// and must end with a `?`.
#[derive(Debug, Clone)]
pub struct PredicatePrefix {
    /// `NamePrefix`.
    name_prefixes: Vec<String>,
    /// `ForbiddenPrefixes`.
    forbidden_prefixes: Vec<String>,
    /// `AllowedMethods`.
    allowed_methods: Vec<String>,
    /// `MethodDefinitionMacros`.
    method_definition_macros: Vec<String>,
    /// `UseSorbetSigs`.
    use_sorbet_sigs: bool,
    /// Spans of `def`/`defs` nodes (and, in principle, any other statement)
    /// whose immediately preceding sibling in the same `StatementsNode` is a
    /// Sorbet `sig` block returning `T::Boolean`. Only populated when
    /// `use_sorbet_sigs` is set, since it needs a whole-tree pre-pass.
    boolean_sig_targets: HashSet<Span>,
}

impl Rule for PredicatePrefix {
    const META: RuleMeta = RuleMeta {
        name: "Naming/PredicatePrefix",
        department: Department::Naming,
        summary: "Checks that predicate method names end with a question mark and do not start \
                  with a forbidden prefix.",
        explanation: "\
A method is determined to be a predicate method if its name starts with one \
of the prefixes listed in the `NamePrefix` configuration. The list defaults \
to `is_`, `has_`, `have_`, and `does_` but may be overridden.

Predicate methods must end with a question mark.

When `ForbiddenPrefixes` is also set (as it is by default), predicate \
methods which begin with a forbidden prefix are not allowed, even if they \
end with a `?`. These methods should be changed to remove the prefix.

When `UseSorbetSigs` is set to `true` (optional), the cop only reports \
offenses if the method has a Sorbet `sig` with a return type of \
`T::Boolean`.

```ruby
# bad
def is_even(value)
end

# good (ForbiddenPrefixes: ['is_'])
def even?(value)
end

# good (ForbiddenPrefixes: [])
def is_even?(value)
end
```

With `AllowedMethods: ['is_a?']` (the default):

```ruby
# good, despite starting with the `is_` prefix
def is_a?(value)
end
```

With `MethodDefinitionMacros: ['define_method', 'define_singleton_method']` \
(the default):

```ruby
# bad
define_method(:is_even) { |value| }

# good
define_method(:even?) { |value| }
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::DefNode],
        config: &[
            linter::ConfigOption {
                name: "NamePrefix",
                default: linter::ConfigDefault::StrList(&["is_", "has_", "have_", "does_"]),
                allowed: &[],
                doc: "Predicate name prefixes.",
            },
            linter::ConfigOption {
                name: "ForbiddenPrefixes",
                default: linter::ConfigDefault::StrList(&["is_", "has_", "have_", "does_"]),
                allowed: &[],
                doc: "Predicate name prefixes that should be removed.",
            },
            linter::ConfigOption {
                name: "AllowedMethods",
                default: linter::ConfigDefault::StrList(&["is_a?"]),
                allowed: &[],
                doc: "Predicate names which, despite having a forbidden prefix, or no `?`, \
                      should still be accepted.",
            },
            linter::ConfigOption {
                name: "MethodDefinitionMacros",
                default: linter::ConfigDefault::StrList(&[
                    "define_method",
                    "define_singleton_method",
                ]),
                allowed: &[],
                doc: "Method definition macros for dynamically generated methods.",
            },
            linter::ConfigOption {
                name: "UseSorbetSigs",
                default: linter::ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Use Sorbet's `T::Boolean` return type to detect predicate methods.",
            },
        ],
        blind_spots: "\
`AllCops/UseProjectIndex` cross-file override detection is not ported: it \
needs the `rubydex` gem's project index, which elysium has no equivalent \
of. Methods that override an ancestor method defined elsewhere in the \
project are always reported here, even when upstream would suppress them.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let name_prefixes = options.str_list("NamePrefix");
        let forbidden_prefixes = options.str_list("ForbiddenPrefixes");
        for forbidden in &forbidden_prefixes {
            if !name_prefixes.contains(forbidden) {
                return Err(options.error(
                    "NamePrefix",
                    format!(
                        "The `Naming/PredicatePrefix` cop is misconfigured. Prefix {forbidden} \
                         must be included in NamePrefix because it is included in \
                         ForbiddenPrefixes."
                    ),
                ));
            }
        }
        let use_sorbet_sigs = match options.get("UseSorbetSigs") {
            Some(value) => {
                value.as_bool().or_else(|| value.as_str().map(|s| s == "true")).unwrap_or(false)
            }
            None => options.bool("UseSorbetSigs"),
        };
        Ok(Self {
            name_prefixes,
            forbidden_prefixes,
            allowed_methods: options.str_list("AllowedMethods"),
            method_definition_macros: options.str_list("MethodDefinitionMacros"),
            use_sorbet_sigs,
            boolean_sig_targets: HashSet::new(),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        if !self.use_sorbet_sigs {
            return;
        }
        let root = ctx.parsed().root();
        let mut targets = HashSet::new();
        collect_boolean_sig_targets(&root, ctx, &mut targets);
        self.boolean_sig_targets = targets;
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => self.check_dynamic_method_define(node, ctx),
            NodeKind::DefNode => self.check_def(node, ctx),
            _ => {}
        }
    }
}

impl PredicatePrefix {
    /// `on_send`'s `dynamic_method_define`: `(send nil? #method_definition_macro? (sym $_) ...)`.
    fn check_dynamic_method_define(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_some() {
            return;
        }
        let macro_name = call.name();
        if !self.method_definition_macros.iter().any(|m| m.as_bytes() == macro_name.as_slice()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(first) = arguments.arguments().iter().next() else { return };
        let Some(symbol) = first.as_symbol_node() else { return };
        let method_name = String::from_utf8_lossy(symbol.unescaped()).into_owned();
        let span = first.span();

        for prefix in &self.name_prefixes {
            if self.allowed_method_name(&method_name, prefix) {
                continue;
            }
            let new_name = self.expected_name(&method_name, prefix);
            ctx.report(&Self::META, span, format!("Rename `{method_name}` to `{new_name}`."));
        }
    }

    /// `on_def`/`on_defs`.
    fn check_def(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        let method_name = String::from_utf8_lossy(def.name().as_slice()).into_owned();
        let has_boolean_sig = self.boolean_sig_targets.contains(&node.span());
        let span = def.name_loc().span();

        for prefix in &self.name_prefixes {
            if self.allowed_method_name(&method_name, prefix) {
                continue;
            }
            if self.use_sorbet_sigs && !has_boolean_sig {
                continue;
            }
            let new_name = self.expected_name(&method_name, prefix);
            ctx.report(&Self::META, span, format!("Rename `{method_name}` to `{new_name}`."));
        }
    }

    /// RuboCop's `allowed_method_name?`.
    fn allowed_method_name(&self, method_name: &str, prefix: &str) -> bool {
        let matches_prefix = method_name
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.as_bytes().first().is_some_and(|&b| !b.is_ascii_digit()));
        !matches_prefix
            || method_name == self.expected_name(method_name, prefix)
            || method_name.ends_with('=')
            || self.allowed_methods.iter().any(|m| m == method_name)
    }

    /// RuboCop's `expected_name`.
    fn expected_name(&self, method_name: &str, prefix: &str) -> String {
        let mut new_name = if self.forbidden_prefixes.iter().any(|p| p == prefix) {
            method_name.replacen(prefix, "", 1)
        } else {
            method_name.to_string()
        };
        if !method_name.ends_with('?') {
            new_name.push('?');
        }
        new_name
    }
}

/// Walks the whole tree once, recording -- for every `StatementsNode` --
/// which immediate children are directly preceded by a Sorbet
/// `sig { returns(T::Boolean) }`-shaped sibling (upstream's `node.left_sibling`
/// plus `sorbet_sig?`).
fn collect_boolean_sig_targets<'pr>(
    node: &Node<'pr>,
    ctx: &Context<'_>,
    targets: &mut HashSet<Span>,
) {
    if let Some(stmts) = node.as_statements_node() {
        let mut previous: Option<Node<'pr>> = None;
        for child in &stmts.body() {
            if let Some(prev) = previous {
                if is_boolean_sorbet_sig(&prev, ctx) {
                    targets.insert(child.span());
                }
            }
            previous = Some(child);
        }
    }
    for_each_child(node, |child| collect_boolean_sig_targets(child, ctx, targets));
}

/// RuboCop's `sorbet_sig?(node, return_type: 'T::Boolean')` matched against
/// `sorbet_return_type`: `(block (send nil? :sig) args (send _ :returns $_type))`.
fn is_boolean_sorbet_sig(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.receiver().is_some() || call.arguments().is_some() {
        return false;
    }
    if call.name().as_slice() != b"sig" {
        return false;
    }
    let Some(block) = call.block() else { return false };
    let Some(block) = block.as_block_node() else { return false };
    let Some(body) = block.body() else { return false };
    let Some(stmts) = body.as_statements_node() else { return false };

    let mut iter = stmts.body().iter();
    let Some(only_statement) = iter.next() else { return false };
    if iter.next().is_some() {
        return false;
    }
    let Some(returns_call) = only_statement.as_call_node() else { return false };
    if returns_call.name().as_slice() != b"returns" {
        return false;
    }
    let Some(returns_args) = returns_call.arguments() else { return false };
    let mut arg_iter = returns_args.arguments().iter();
    let Some(type_arg) = arg_iter.next() else { return false };
    if arg_iter.next().is_some() {
        return false;
    }
    ctx.text(type_arg.span()) == b"T::Boolean"
}

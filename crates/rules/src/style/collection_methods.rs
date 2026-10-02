//! `Style/CollectionMethods`, ported from RuboCop's
//! `lib/rubocop/cop/style/collection_methods.rb`, plus the
//! `MethodPreference` mixin it includes.
//!
//! Upstream dispatches `on_block`/`on_numblock`/`on_itblock` (all aliased
//! to the same handler) separately from `on_send`, because whitequark
//! represents a call with a literal block as its own wrapping node. Prism
//! keeps such a call as a plain [`NodeKind::CallNode`] whose `block()`
//! accessor reaches the attached block (same unification already used by
//! `Style/DataInheritance` and `Style/CollectionQuerying`), so a single
//! `CallNode` handler covers both paths here: a call with any kind of
//! block attached is always eligible (`on_block`'s unconditional check),
//! while a bare call additionally needs [`implicit_block`]'s guard.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG_TEMPLATE: &str = "Prefer `%<prefer>s` over `%<current>s`.";

/// RuboCop's default `PreferredMethods`.
const DEFAULT_PREFERRED: &[(&str, &str)] = &[
    ("collect", "map"),
    ("collect!", "map!"),
    ("collect_concat", "flat_map"),
    ("inject", "reduce"),
    ("detect", "find"),
    ("find_all", "select"),
    ("member?", "include?"),
];

/// RuboCop's default `MethodsAcceptingSymbol`.
const DEFAULT_ACCEPTING_SYMBOL: &[&str] = &["inject", "reduce"];

/// Enforces the use of consistent method names from the `Enumerable` module.
#[derive(Debug, Clone)]
pub struct CollectionMethods {
    preferred_methods: Vec<(String, String)>,
    methods_accepting_symbol: Vec<String>,
}

impl Rule for CollectionMethods {
    const META: RuleMeta = RuleMeta {
        name: "Style/CollectionMethods",
        department: Department::Style,
        summary: "Enforces the use of consistent method names from the `Enumerable` module.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let preferred_methods =
            options.get("PreferredMethods").and_then(OptionValue::as_map).map_or_else(
                || {
                    DEFAULT_PREFERRED
                        .iter()
                        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                        .collect()
                },
                |entries| {
                    entries
                        .iter()
                        .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                        .collect()
                },
            );
        let methods_accepting_symbol = options
            .get("MethodsAcceptingSymbol")
            .map(OptionValue::to_string_list)
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| DEFAULT_ACCEPTING_SYMBOL.iter().map(|s| (*s).to_string()).collect());
        Ok(Self { preferred_methods, methods_accepting_symbol })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.block().is_none() && !self.is_implicit_block(&call) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let name = String::from_utf8_lossy(ctx.text(selector.span())).into_owned();
        let Some(preferred) = self.preferred_methods.iter().find(|(k, _)| *k == name) else {
            return;
        };
        let preferred = preferred.1.clone();

        let message = MSG_TEMPLATE.replace("%<prefer>s", &preferred).replace("%<current>s", &name);
        let edits = vec![Edit::replace(selector.span(), preferred.into_bytes())];
        ctx.report_with_fix(
            &Self::META,
            selector.span(),
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

impl CollectionMethods {
    /// `implicit_block?`: at least one argument, and the last one is
    /// either a block-pass (`&:sym`/`&proc`) or a bare symbol for a method
    /// listed in `MethodsAcceptingSymbol`.
    fn is_implicit_block(&self, call: &CallNode<'_>) -> bool {
        let Some(args) = call.arguments() else { return false };
        let args = args.arguments();
        let Some(last) = args.iter().last() else { return false };
        if last.kind() == NodeKind::BlockArgumentNode {
            return true;
        }
        last.kind() == NodeKind::SymbolNode
            && self.methods_accepting_symbol.iter().any(|m| call.name().as_slice() == m.as_bytes())
    }
}

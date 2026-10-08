//! `Lint/UnexpectedBlockArity`, ported from RuboCop's
//! `lib/rubocop/cop/lint/unexpected_block_arity.rb`.
//!
//! RuboCop's `on_block`/`on_numblock`/`on_itblock` all fire on whitequark's
//! unified block-with-its-call node, and `add_offense(node)` spans that
//! whole construct (receiver through the block's closing delimiter). Prism
//! instead attaches a literal block to the [`ruby_ast::node::CallNode`] it
//! belongs to via `call.block()`, whose own span already covers the entire
//! `receiver.method { ... }`/`receiver.method do ... end` text, so this rule
//! dispatches on [`NodeKind::CallNode`] directly and reports on the call
//! node's own span.
//!
//! The `Methods` config option is a `Hash[String, Integer]`
//! (`linter::OptionValue::Map`); a user override (every fixture here sets
//! one) is read through that map, falling back to upstream's
//! `config/default.yml` table (`DEFAULT_METHODS` below) when absent, since
//! [`linter::ConfigDefault`] has no map variant to express it statically.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, OptionValue,
    Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::BlockNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Upstream's `config/default.yml` `Methods` table.
const DEFAULT_METHODS: &[(&str, i64)] = &[
    ("chunk_while", 2),
    ("each_with_index", 2),
    ("each_with_object", 2),
    ("inject", 2),
    ("max", 2),
    ("min", 2),
    ("minmax", 2),
    ("reduce", 2),
    ("slice_when", 2),
    ("sort", 2),
];

/// Looks for blocks that have fewer arguments that the calling method expects.
#[derive(Debug, Clone)]
pub struct UnexpectedBlockArity {
    /// Method name -> minimum expected positional arity.
    methods: Vec<(String, i64)>,
}

impl UnexpectedBlockArity {
    /// Upstream's `included_method?`/`expected_arity`.
    fn expected_arity(&self, name: &[u8]) -> Option<i64> {
        self.methods.iter().find(|(method, _)| method.as_bytes() == name).map(|(_, arity)| *arity)
    }
}

impl Rule for UnexpectedBlockArity {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UnexpectedBlockArity",
        department: Department::Lint,
        summary: "Looks for blocks that have fewer arguments that the calling method expects.",
        explanation: "Checks for a block that is known to need more positional \
            block arguments than are given (by default this is configured for `Enumerable` \
            methods needing 2 arguments). Optional arguments are allowed, although they \
            don't generally make sense as the default value will be used. Blocks that have \
            no receiver, or take splatted arguments (ie. `*args`) are always accepted.\n\n\
            Keyword arguments (including `**kwargs`) do not get counted towards this, as \
            they are not used by the methods in question.",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "Methods",
            default: ConfigDefault::Nil,
            allowed: &[],
            doc: "Method names and their expected minimum positional block arity.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let methods = match options.get("Methods").and_then(OptionValue::as_map) {
            Some(entries) => entries
                .iter()
                .filter_map(|(name, value)| value.as_int().map(|arity| (name.clone(), arity)))
                .collect(),
            None => {
                DEFAULT_METHODS.iter().map(|&(name, arity)| (name.to_string(), arity)).collect()
            }
        };
        Ok(Self { methods })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        if call.receiver().is_none() {
            return;
        }
        let Some(block_node) = call.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };
        let Some(expected) = self.expected_arity(call.name().as_slice()) else { return };
        let Some(actual) = arg_count(&block) else { return };
        if actual >= expected {
            return;
        }
        let method = String::from_utf8_lossy(call.name().as_slice());
        let message =
            format!("`{method}` expects at least {expected} positional arguments, got {actual}.");
        ctx.report(&Self::META, node.span(), message);
    }
}

/// Upstream's `arg_count`: `None` for a splatted (`*args`) parameter, which
/// upstream represents as `Float::INFINITY` -- always accepted, so this
/// short-circuits as "never offends" instead of comparing a real count.
fn arg_count(block: &BlockNode<'_>) -> Option<i64> {
    let Some(params) = block.parameters() else { return Some(0) };
    match params.kind() {
        NodeKind::NumberedParametersNode => {
            Some(i64::from(params.as_numbered_parameters_node().expect("kind matched").maximum()))
        }
        NodeKind::ItParametersNode => Some(1),
        NodeKind::BlockParametersNode => {
            let block_params = params.as_block_parameters_node().expect("kind matched");
            let Some(params) = block_params.parameters() else { return Some(0) };
            if params.rest().is_some() {
                return None;
            }
            let count = params.requireds().len() + params.optionals().len();
            Some(i64::try_from(count).unwrap_or(i64::MAX))
        }
        _ => Some(0),
    }
}

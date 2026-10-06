//! `Performance/FlatMap`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/flat_map.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `flat_map` instead of `%method%...%flatten%`.";
const FLATTEN_MULTIPLE_LEVELS: &str = " Beware, `flat_map` only flattens 1 level and `flatten` can be used to flatten multiple levels.";

/// Identifies usages of `map { ... }.flatten` and change them to use
/// `flat_map { ... }` instead.
#[derive(Debug, Clone)]
pub struct FlatMap {
    enabled_for_flatten_without_params: bool,
}

/// `flatten_level, = *params.first`: `None` when there is no first
/// parameter or its first child is nil (childless nodes, receiverless calls).
fn flatten_level_is_one(first: Option<&Node<'_>>) -> (bool, bool) {
    let Some(first) = first else { return (false, false) };
    match first.kind() {
        NodeKind::NilNode | NodeKind::TrueNode | NodeKind::FalseNode | NodeKind::SelfNode => {
            (false, false)
        }
        NodeKind::CallNode => {
            let has_receiver = first.as_call_node().is_some_and(|c| c.receiver().is_some());
            (has_receiver, false)
        }
        NodeKind::IntegerNode => {
            let one = first.as_integer_node().is_some_and(|i| {
                let v: i32 = i.value().try_into().unwrap_or(0);
                v == 1
            });
            (true, one)
        }
        NodeKind::FloatNode => {
            (true, first.as_float_node().is_some_and(|f| (f.value() - 1.0).abs() == 0.0))
        }
        _ => (true, false),
    }
}

impl Rule for FlatMap {
    const META: RuleMeta = RuleMeta {
        name: "Performance/FlatMap",
        department: Department::Performance,
        summary: "Use `Enumerable#flat_map` instead of `Enumerable#map...Array#flatten(1)` or `Enumerable#collect..Array#flatten(1)`.",
        explanation: "Identifies usages of `map { ... }.flatten` and change them to use \
                      `flat_map { ... }` instead.\n\n```ruby\n# bad\n\
                      [1, 2, 3, 4].map { |e| [e, e] }.flatten(1)\n\
                      [1, 2, 3, 4].collect { |e| [e, e] }.flatten(1)\n\n# good\n\
                      [1, 2, 3, 4].flat_map { |e| [e, e] }\n\
                      [1, 2, 3, 4].map { |e| [e, e] }.flatten\n\
                      [1, 2, 3, 4].collect { |e| [e, e] }.flatten\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnabledForFlattenWithoutParams",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Also register `flatten` without a level argument.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { enabled_for_flatten_without_params: options.bool("EnabledForFlattenWithoutParams") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let flatten = call.name();
        let flatten = flatten.as_slice();
        if flatten != b"flatten" && flatten != b"flatten!" {
            return;
        }
        let Some(map_call) = call.receiver().and_then(|r| r.as_call_node()) else { return };
        let first_method = map_call.name();
        if !matches!(first_method.as_slice(), b"map" | b"collect") || map_call.arguments().is_some()
        {
            return;
        }
        // `(block (call _ map) ...)` or `(call _ map (block_pass _))`.
        let Some(map_block) = map_call.block() else { return };
        // whitequark `numblock`/`itblock` do not match `(block ...)`.
        if map_block.as_block_node().is_some_and(|block| {
            block.parameters().is_some_and(|p| {
                p.as_numbered_parameters_node().is_some() || p.as_it_parameters_node().is_some()
            })
        }) {
            return;
        }
        let map_node = call.receiver().expect("receiver checked above");
        let Some(selector) = map_call.message_loc() else { return };

        let params = call.arguments();
        let first_param = params.as_ref().and_then(|a| a.arguments().iter().next()).or_else(|| {
            call.block().filter(|b| b.as_block_argument_node().is_some())
        });
        let (has_level, is_one) = flatten_level_is_one(first_param.as_ref());

        let message = if self.enabled_for_flatten_without_params && !has_level {
            format!("{MSG}{FLATTEN_MULTIPLE_LEVELS}")
        } else if is_one {
            MSG.to_string()
        } else {
            return;
        };
        let message = message
            .replace("%method%", &String::from_utf8_lossy(first_method.as_slice()))
            .replace("%flatten%", &String::from_utf8_lossy(flatten));

        let node_end = ruby_ast::ext::call_span_excluding_block(&call).end;
        let range = Span::new(selector.span().start, node_end);
        if has_level {
            let map_end = map_node.span().end;
            ctx.report_with_fix(
                &Self::META,
                range,
                message,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![
                        Edit::delete(Span::new(map_end, node_end)),
                        Edit::replace(selector.span(), b"flat_map".to_vec()),
                    ],
                },
            );
        } else {
            ctx.report(&Self::META, range, message);
        }
    }
}

//! `Sorbet/SignatureBuildOrder`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/signatures/signature_build_order.rb`.

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::node::{BlockNode, CallNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Checks for the correct order of `sig` builder methods.
#[derive(Debug, Clone)]
pub struct SignatureBuildOrder {
    indexes: HashMap<String, usize>,
}

impl Rule for SignatureBuildOrder {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/SignatureBuildOrder",
        department: Department::Sorbet,
        summary: "Enforces the order of parts in a signature.",
        explanation: "Checks for the correct order of `sig` builder methods.\n\n```ruby\n# bad\nsig { void.abstract }\n\n# good\nsig { abstract.void }\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[linter::ConfigOption {
            name: "Order",
            default: linter::ConfigDefault::StrList(&[
                "final",
                "abstract",
                "implementation",
                "override",
                "overridable",
                "type_parameters",
                "params",
                "bind",
                "returns",
                "void",
                "soft",
                "checked",
                "on_failure",
            ]),
            allowed: &[],
            doc: "The order in which to enforce the builder methods are called.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mut indexes = HashMap::new();
        for (index, name) in options.str_list("Order").into_iter().enumerate() {
            indexes.insert(name, index);
        }
        Ok(Self { indexes })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if !is_signature(&call) {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let Some(body) = single_body(&block) else { return };

        // `call_chain`: foo.bar.baz => [foo, foo.bar, foo.bar.baz].
        let mut chain: Vec<CallNode<'_>> = Vec::new();
        let body_span = body.span();
        let mut current = Some(body);
        while let Some(n) = current {
            let Some(c) = n.as_call_node() else { break };
            if c.is_safe_navigation() || !is_send_like(&c) {
                break;
            }
            current = c.receiver();
            chain.push(c);
        }
        chain.reverse();

        // (actual index, expected index)
        let entries: Vec<(usize, Option<usize>)> = chain
            .iter()
            .enumerate()
            .map(|(actual, c)| {
                let name = String::from_utf8_lossy(c.name().as_slice()).into_owned();
                (actual, self.indexes.get(&name).copied())
            })
            .collect();
        let mut expected: Vec<usize> =
            entries.iter().filter(|(_, e)| e.is_some()).map(|(a, _)| *a).collect();
        expected.sort_by_key(|a| entries[*a].1);
        for (actual, e) in &entries {
            if e.is_none() {
                expected.insert(*actual, *actual);
            }
        }

        let name_of = |i: usize| String::from_utf8_lossy(chain[i].name().as_slice()).into_owned();
        let expected_names: Vec<String> = expected.iter().map(|i| name_of(*i)).collect();
        let actual_names: Vec<String> = (0..chain.len()).map(name_of).collect();
        if expected_names == actual_names {
            return;
        }

        let mut source: Option<String> = None;
        for &i in &expected {
            let c = &chain[i];
            let mut args: Vec<String> = Vec::new();
            if let Some(arguments) = c.arguments() {
                for arg in &arguments.arguments() {
                    args.push(String::from_utf8_lossy(ctx.text(arg.span())).into_owned());
                }
            }
            if let Some(block_arg) = c.block().filter(|b| b.as_block_argument_node().is_some()) {
                args.push(String::from_utf8_lossy(ctx.text(block_arg.span())).into_owned());
            }
            let send = if args.is_empty() {
                name_of(i)
            } else {
                format!("{}({})", name_of(i), args.join(", "))
            };
            source = Some(match source {
                Some(receiver) => format!("{receiver}.{send}"),
                None => send,
            });
        }
        ctx.report_with_fix(
            &Self::META,
            body_span,
            format!(
                "Sig builders must be invoked in the following order: {}.",
                expected_names.join(", ")
            ),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(body_span, source.unwrap_or_default().into_bytes())],
            },
        );
    }
}

/// `Sorbet::SignatureHelp#signature?`: `sig`, `sig(:final)`, `T::Sig.sig`...
/// called with a literal block taking no parameters (whitequark `block`, not
/// `numblock`/`itblock`).
fn is_signature(call: &CallNode<'_>) -> bool {
    if call.is_safe_navigation() || call.name().as_slice() != b"sig" {
        return false;
    }
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return false };
    if let Some(params) = block.parameters() {
        let Some(params) = params.as_block_parameters_node() else { return false };
        if params.parameters().is_some() || params.locals().iter().next().is_some() {
            return false;
        }
    }
    if let Some(arguments) = call.arguments() {
        let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let [only] = args.as_slice() else { return false };
        let Some(sym) = only.as_symbol_node() else { return false };
        if sym.unescaped() != b"final" {
            return false;
        }
    }
    match call.receiver() {
        None => true,
        Some(receiver) => {
            matches!(const_name(&receiver).as_deref(), Some("T::Sig" | "T::Sig::WithoutRuntime"))
        }
    }
}

/// The block's body when it is a single statement (whitequark elides the
/// `begin`); `None` for no body or several statements.
fn single_body<'a>(block: &BlockNode<'a>) -> Option<Node<'a>> {
    let body = block.body()?;
    let statements = body.as_statements_node()?;
    let mut iter = statements.body().iter();
    let first = iter.next()?;
    iter.next().is_none().then_some(first)
}

/// A `send`/`csend` node: a call without a literal block.
fn is_send_like(call: &CallNode<'_>) -> bool {
    call.block().is_none_or(|b| b.as_block_node().is_none())
}

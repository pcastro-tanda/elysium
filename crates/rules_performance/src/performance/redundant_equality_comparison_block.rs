//! `Performance/RedundantEqualityComparisonBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/redundant_equality_comparison_block.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind, each_descendant};
use ruby_source::Span;

const TARGET_METHODS: &[&[u8]] = &[b"all?", b"any?", b"one?", b"none?"];
const COMPARISON_METHODS: &[&[u8]] = &[b"==", b"===", b"is_a?", b"kind_of?"];
const REGEXP_METHODS: &[&[u8]] = &[b"=~", b"match?"];
const IS_A_METHODS: &[&[u8]] = &[b"is_a?", b"kind_of?"];

/// Checks for uses `Enumerable#all?`, `Enumerable#any?`, `Enumerable#one?`, and
/// `Enumerable#none?` compared with `===` or similar methods in a block.
#[derive(Debug, Clone)]
pub struct RedundantEqualityComparisonBlock {
    allow_regexp_match: bool,
    target_ruby_version: f32,
}

/// The block's only parameter node (`arguments.one?`), or `None`.
fn sole_parameter<'pr>(block: &BlockNode<'pr>) -> Option<Node<'pr>> {
    let params = block.parameters()?.as_block_parameters_node()?;
    let mut all: Vec<Node<'pr>> = Vec::new();
    if let Some(p) = params.parameters() {
        all.extend(p.requireds().iter());
        all.extend(p.optionals().iter());
        all.extend(p.rest());
        all.extend(p.posts().iter());
        all.extend(p.keywords().iter());
        all.extend(p.keyword_rest());
        all.extend(p.block().map(|b| b.as_node()));
    }
    all.extend(params.locals().iter());
    if all.len() == 1 { all.pop() } else { None }
}

/// rubocop-ast's `Node#receiver` for an arbitrary node:
/// `{(send $_ ...) (any_block (call $_ ...) ...)}`.
fn generic_receiver<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() && call.block().is_none_or(|b| b.as_block_node().is_none()) {
        return None;
    }
    call.receiver()
}

fn first_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    call.arguments()?.arguments().iter().next()
}

impl RedundantEqualityComparisonBlock {
    fn use_block_argument_in_method_argument_of_operand(
        block_argument: &[u8],
        operand: &Node<'_>,
        ctx: &Context<'_>,
    ) -> bool {
        let Some(call) = operand.as_call_node() else { return false };
        if call.is_safe_navigation() || call.block().is_some_and(|b| b.as_block_node().is_some()) {
            return false;
        }
        let Some(args) = call.arguments() else { return false };
        for arg in &args.arguments() {
            if ctx.text(arg.span()) == block_argument {
                return true;
            }
            let mut found = false;
            each_descendant(&arg, &mut |d: &Node<'_>| {
                if d.as_local_variable_read_node().is_some() && ctx.text(d.span()) == block_argument
                {
                    found = true;
                }
            });
            if found {
                return true;
            }
        }
        false
    }
}

impl Rule for RedundantEqualityComparisonBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RedundantEqualityComparisonBlock",
        department: Department::Performance,
        summary: "Checks for uses `Enumerable#all?`, `Enumerable#any?`, `Enumerable#one?`, or `Enumerable#none?` are compared with `===` or similar methods in block.",
        explanation: "Checks for uses `Enumerable#all?`, `Enumerable#any?`, `Enumerable#one?`, and `Enumerable#none?` compared with `===` or similar methods in a block.\n\nBy default, `Object#===` behaves the same as `Object#==`, but this behavior is overridden in subclasses (for example `Range#===`). `AllowRegexpMatch` is true by default because `regexp.match?('string')` in a block often changes to the opposite result when replaced by an argument.\n\nThis cop is unsafe because `===` and `==` do not always behave the same.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "AllowRegexpMatch",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Do not flag blocks using `=~` or `match?`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_regexp_match: options.bool("AllowRegexpMatch"),
            target_ruby_version: options.target_ruby_version(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.target_ruby_version < 2.5 {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let method_name = call.name();
        let method_name = method_name.as_slice();
        if !TARGET_METHODS.contains(&method_name) {
            return;
        }
        let Some(block_node) = call.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };
        let Some(param) = sole_parameter(&block) else { return };
        let params_text = block.parameters().map_or(&b""[..], |p| ctx.text(p.span()));
        if params_text.contains(&b',') {
            return;
        }
        let block_argument = ctx.text(param.span());

        let Some(body) = block.body() else { return };
        let Some(statements) = body.as_statements_node() else { return };
        let mut stmts = statements.body().iter();
        let (Some(body), None) = (stmts.next(), stmts.next()) else { return };
        let Some(body_call) = body.as_call_node() else { return };
        if body_call.is_safe_navigation() {
            return;
        }
        let body_name = body_call.name();
        let body_name = body_name.as_slice();
        if !(COMPARISON_METHODS.contains(&body_name)
            || (!self.allow_regexp_match && REGEXP_METHODS.contains(&body_name)))
        {
            return;
        }
        let Some(receiver) = body_call.receiver() else { return };
        let Some(first_arg) = first_argument(&body_call) else { return };
        let receiver_src = ctx.text(receiver.span());
        let first_arg_src = ctx.text(first_arg.span());

        // same_block_argument_and_is_a_argument?
        let same = if body_name == b"===" {
            block_argument != first_arg_src
        } else if IS_A_METHODS.contains(&body_name) {
            block_argument == first_arg_src
        } else {
            generic_receiver(&first_arg).is_some_and(|r| ctx.text(r.span()) == receiver_src)
        };
        if same {
            return;
        }

        // new_argument
        let new_argument = if block_argument == receiver_src {
            if Self::use_block_argument_in_method_argument_of_operand(block_argument, &first_arg, ctx)
            {
                return;
            }
            first_arg_src
        } else if block_argument == first_arg_src {
            if Self::use_block_argument_in_method_argument_of_operand(block_argument, &receiver, ctx)
            {
                return;
            }
            receiver_src
        } else {
            return;
        };

        let Some(selector) = call.message_loc() else { return };
        let range = Span::new(selector.span().start, block.as_node().span().end);
        let prefer = format!(
            "{}({})",
            String::from_utf8_lossy(method_name),
            String::from_utf8_lossy(new_argument)
        );
        let message = format!("Use `{prefer}` instead of block.");
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, prefer.into_bytes())],
            },
        );
    }
}

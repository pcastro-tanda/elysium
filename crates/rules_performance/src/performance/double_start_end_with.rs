//! `Performance/DoubleStartEndWith`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/double_start_end_with.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const METHODS: [&[u8]; 2] = [b"start_with?", b"end_with?"];
const METHODS_WITH_ACTIVE_SUPPORT: [&[u8]; 4] =
    [b"start_with?", b"starts_with?", b"end_with?", b"ends_with?"];

/// Checks for consecutive `#start_with?` or `#end_with?` calls. These methods
/// accept multiple arguments, so in some cases like when they are separated by
/// `||`, they can be combined into a single method call.
#[derive(Debug, Clone)]
pub struct DoubleStartEndWith {
    include_active_support_aliases: bool,
}

/// `Node#pure?` from rubocop-ast.
fn is_pure(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::SourceFileNode
        | NodeKind::SourceLineNode
        | NodeKind::ConstantReadNode
        | NodeKind::ConstantPathNode
        | NodeKind::ClassVariableReadNode
        | NodeKind::DefinedNode
        | NodeKind::FalseNode
        | NodeKind::FloatNode
        | NodeKind::GlobalVariableReadNode
        | NodeKind::InstanceVariableReadNode
        | NodeKind::LocalVariableReadNode
        | NodeKind::NilNode
        | NodeKind::StringNode
        | NodeKind::SymbolNode
        | NodeKind::TrueNode
        | NodeKind::IntegerNode => true,
        NodeKind::AndNode
        | NodeKind::OrNode
        | NodeKind::ArrayNode
        | NodeKind::ParenthesesNode
        | NodeKind::StatementsNode
        | NodeKind::CaseNode
        | NodeKind::WhenNode
        | NodeKind::IfNode
        | NodeKind::UnlessNode
        | NodeKind::ElseNode
        | NodeKind::InterpolatedStringNode
        | NodeKind::InterpolatedSymbolNode
        | NodeKind::InterpolatedRegularExpressionNode
        | NodeKind::RegularExpressionNode
        | NodeKind::EmbeddedStatementsNode
        | NodeKind::EmbeddedVariableNode
        | NodeKind::RangeNode
        | NodeKind::HashNode
        | NodeKind::KeywordHashNode
        | NodeKind::AssocNode
        | NodeKind::WhileNode
        | NodeKind::UntilNode => {
            let mut pure = true;
            ruby_ast::for_each_child(node, |child| pure = pure && is_pure(child));
            pure
        }
        NodeKind::BeginNode => {
            node.as_begin_node()
                .is_some_and(|b| b.rescue_clause().is_none() && b.ensure_clause().is_none())
                && {
                    let mut pure = true;
                    ruby_ast::for_each_child(node, |child| pure = pure && is_pure(child));
                    pure
                }
        }
        _ => false,
    }
}

struct Matched<'pr> {
    receiver: Node<'pr>,
    method: Vec<u8>,
    safe_navigation: bool,
    first_args: Vec<Node<'pr>>,
    second_args: Vec<Node<'pr>>,
}

impl DoubleStartEndWith {
    fn methods(&self) -> &'static [&'static [u8]] {
        if self.include_active_support_aliases { &METHODS_WITH_ACTIVE_SUPPORT } else { &METHODS }
    }

    /// `(call $_recv [%methods_to_check $_method] $...)` and `(call _recv _method $...)`.
    fn two_calls<'pr>(
        &self,
        first: &Node<'pr>,
        second: &Node<'pr>,
        ctx: &Context<'_>,
    ) -> Option<Matched<'pr>> {
        let first = first.as_call_node()?;
        let second = second.as_call_node()?;
        // A call carrying a block is a `block` node in whitequark.
        if first.block().is_some() || second.block().is_some() {
            return None;
        }
        let method = first.name();
        if !self.methods().contains(&method.as_slice()) || second.name().as_slice() != method.as_slice()
        {
            return None;
        }
        let receiver = first.receiver()?;
        let second_receiver = second.receiver()?;
        if ctx.text(receiver.span()) != ctx.text(second_receiver.span()) {
            return None;
        }
        let first_args: Vec<Node<'pr>> =
            first.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        let second_args: Vec<Node<'pr>> =
            second.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        Some(Matched {
            receiver,
            method: method.as_slice().to_vec(),
            safe_navigation: first.is_safe_navigation(),
            first_args,
            second_args,
        })
    }

    fn check(node: &Node<'_>, matched: &Matched<'_>, bang: &str, ctx: &mut Context<'_>) {
        let (Some(first_arg), Some(last_arg)) =
            (matched.first_args.first(), matched.second_args.last())
        else {
            return;
        };
        if !matched.second_args.iter().all(is_pure) {
            return;
        }
        let combined_args = matched
            .first_args
            .iter()
            .chain(matched.second_args.iter())
            .map(|arg| String::from_utf8_lossy(ctx.text(arg.span())).into_owned())
            .collect::<Vec<_>>()
            .join(", ");
        let dot = if matched.safe_navigation { "&." } else { "." };
        let replacement = format!(
            "{bang}{}{dot}{}({combined_args})",
            String::from_utf8_lossy(ctx.text(matched.receiver.span())),
            String::from_utf8_lossy(&matched.method),
        );
        let message = format!(
            "Use `{replacement}` instead of `{}`.",
            String::from_utf8_lossy(ctx.text(node.span()))
        );
        let range = Span::new(first_arg.span().start, last_arg.span().end);
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, combined_args.into_bytes())],
            },
        );
    }
}

/// `(send (call ...) :!)`, returning the inner call; the `!` must be spelled `!`
/// for `prefix_bang?`.
fn negated<'pr>(node: &Node<'pr>, ctx: &Context<'_>) -> Option<(Node<'pr>, bool)> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"!"
        || call.arguments().is_some()
        || call.block().is_some()
        || call.is_safe_navigation()
    {
        return None;
    }
    let inner = call.receiver()?;
    let prefix_bang = call.message_loc().is_some_and(|loc| ctx.text(loc.span()) == b"!");
    Some((inner, prefix_bang))
}

impl Rule for DoubleStartEndWith {
    const META: RuleMeta = RuleMeta {
        name: "Performance/DoubleStartEndWith",
        department: Department::Performance,
        summary: "Use `str.{start,end}_with?(x, ..., y, ...)` instead of `str.{start,end}_with?(x, ...) || str.{start,end}_with?(y, ...)`.",
        explanation: "Checks for consecutive `#start_with?` or `#end_with?` calls. These methods \
                      accept multiple arguments, so in some cases like when they are separated \
                      by `||`, they can be combined into a single method call.\n\n\
                      `IncludeActiveSupportAliases` configuration option is used to check for \
                      `starts_with?` and `ends_with?`. These methods are defined by Active \
                      Support.\n\n```ruby\n# bad\n\
                      str.start_with?(\"a\") || str.start_with?(Some::CONST)\n\
                      str.start_with?(\"a\", \"b\") || str.start_with?(\"c\")\n\
                      !str.start_with?(foo) && !str.start_with?(bar)\n\
                      str.end_with?(var1) || str.end_with?(var2)\n\n# good\n\
                      str.start_with?(\"a\", Some::CONST)\n\
                      str.start_with?(\"a\", \"b\", \"c\")\n\
                      !str.start_with?(foo, bar)\n\
                      str.end_with?(var1, var2)\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::OrNode, NodeKind::AndNode],
        config: &[ConfigOption {
            name: "IncludeActiveSupportAliases",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Also check `starts_with?` and `ends_with?` (Active Support).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { include_active_support_aliases: options.bool("IncludeActiveSupportAliases") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(or) = node.as_or_node() {
            if let Some(matched) = self.two_calls(&or.left(), &or.right(), ctx) {
                Self::check(node, &matched, "", ctx);
            }
        } else if let Some(and) = node.as_and_node() {
            let (Some((left, bang)), Some((right, _))) =
                (negated(&and.left(), ctx), negated(&and.right(), ctx))
            else {
                return;
            };
            if let Some(matched) = self.two_calls(&left, &right, ctx) {
                Self::check(node, &matched, if bang { "!" } else { "" }, ctx);
            }
        }
    }
}

//! `Sorbet/BuggyObsoleteStrictMemoization`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/buggy_obsolete_strict_memoization.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "This might be a mistaken variant of the two-stage workaround that used to be needed for memoization in `#typed: strict` files. See https://sorbet.org/docs/type-assertions#put-type-assertions-behind-memoization.";

/// Checks for the a mistaken variant of the "obsolete memoization pattern" that used to be required for older Sorbet versions in `#typed: strict` files.
#[derive(Debug, Clone)]
pub struct BuggyObsoleteStrictMemoization {
    /// `relevant_file?`: `sorbet_enabled?` (`sorbet-static` is locked).
    enabled: bool,
    /// Start offsets of `begin ... end` bodies (whitequark `kwbegin` holds
    /// its statements directly, so there is no `begin` node for them).
    kwbegin_bodies: Vec<u32>,
}

impl Rule for BuggyObsoleteStrictMemoization {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/BuggyObsoleteStrictMemoization",
        department: Department::Sorbet,
        summary: "Checks for the a mistaken variant of the \"obsolete memoization pattern\" that used to be required for older Sorbet versions in `#typed: strict` files.",
        explanation: "Checks for the a mistaken variant of the \"obsolete memoization pattern\" that used to be required\nfor older Sorbet versions in `#typed: strict` files. The mistaken variant would overwrite the ivar with `nil`\non every call, causing the memoized value to be discarded and recomputed on every call.\n\nThis cop will correct it to read from the ivar instead of `nil`, which will memoize it correctly.\n\nThe result of this correction will be the \"obsolete memoization pattern\", which can further be corrected by\nthe `Sorbet/ObsoleteStrictMemoization` cop.\n\nSee `Sorbet/ObsoleteStrictMemoization` for more details.\n\nIf the computation being memoized had side effects, calling it only once (instead of once on every call\nto the affected method) can be observed, and might be a breaking change.\n\n```ruby\n# bad\nsig { returns(Foo) }\ndef foo\n  @foo = T.let(nil, T.nilable(Foo))\n  @foo ||= some_computation\nend\n\n# good\nsig { returns(Foo) }\ndef foo\n  @foo = T.let(@foo, T.nilable(Foo))\n  @foo ||= some_computation\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode, NodeKind::BeginNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            enabled: options.gem_version("sorbet-static").is_some(),
            kwbegin_bodies: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled {
            return;
        }
        if let Some(begin) = node.as_begin_node() {
            if begin.begin_keyword_loc().is_some()
                && begin.rescue_clause().is_none()
                && begin.ensure_clause().is_none()
            {
                if let Some(statements) = begin.statements() {
                    self.kwbegin_bodies.push(statements.as_node().span().start);
                }
            }
            return;
        }
        let Some(statements) = node.as_statements_node() else { return };
        let stmts: Vec<Node<'_>> = statements.body().iter().collect();
        let [.., first, second] = stmts.as_slice() else { return };
        if self.kwbegin_bodies.contains(&node.span().start) {
            return;
        }
        let Some((ivar, nil_span)) = buggy_legacy_memoization_pattern(first, second) else {
            return;
        };
        ctx.report_with_fix(
            &Self::META,
            nil_span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(nil_span, ivar.into_bytes())],
            },
        );
    }
}

/// `(const {nil? cbase} :T)`.
fn is_t_const(node: &Node<'_>) -> bool {
    match node {
        Node::ConstantReadNode { .. } => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"T")
        }
        Node::ConstantPathNode { .. } => node.as_constant_path_node().is_some_and(|path| {
            path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"T")
        }),
        _ => false,
    }
}

/// A plain `send` (not `csend`, no attached block) named `name` with
/// exactly `arity` arguments.
fn plain_send<'a>(node: &'a Node<'a>, name: &[u8], arity: usize) -> Option<CallNode<'a>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() || call.name().as_slice() != name || call.block().is_some() {
        return None;
    }
    let count = call.arguments().map_or(0, |args| args.arguments().iter().count());
    (count == arity).then_some(call)
}

/// ```text
/// (begin ...
///   (ivasgn $_ivar
///     (send (const {nil? cbase} :T) :let
///       $nil
///       (send (const {nil? cbase} :T) :nilable _ivar_type)))
///   (or-asgn (ivasgn _ivar) _initialization_expr))
/// ```
fn buggy_legacy_memoization_pattern(
    first: &Node<'_>,
    second: &Node<'_>,
) -> Option<(String, ruby_source::Span)> {
    let write = first.as_instance_variable_write_node()?;
    let ivar = write.name();
    let value = write.value();
    let let_call = plain_send(&value, b"let", 2)?;
    if !is_t_const(&let_call.receiver()?) {
        return None;
    }
    let args: Vec<Node<'_>> = let_call.arguments()?.arguments().iter().collect();
    args[0].as_nil_node()?;
    let nilable = plain_send(&args[1], b"nilable", 1)?;
    if !is_t_const(&nilable.receiver()?) {
        return None;
    }

    let or_write = second.as_instance_variable_or_write_node()?;
    if or_write.name().as_slice() != ivar.as_slice() {
        return None;
    }
    Some((String::from_utf8_lossy(ivar.as_slice()).into_owned(), args[0].span()))
}

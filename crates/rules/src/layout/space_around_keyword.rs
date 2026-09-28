//! `Layout/SpaceAroundKeyword`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_around_keyword.rb`.
//!
//! RuboCop dispatches on whitequark node types and reads keyword locations
//! off `node.loc`; several of those types differ from Prism's:
//!
//! - `unless` produces whitequark's `:if` type (condition negated), so
//!   upstream's single `on_if` handler covers both `if`/`elsif`/ternary and
//!   `unless`. Prism keeps `UnlessNode` separate, so this port handles it
//!   with the same logic as `IfNode`.
//! - Whitequark folds `if`/`elsif`/`else` into one node per branch, with
//!   `loc.else` on a branch pointing at the *next* branch's keyword (so an
//!   `elsif` gets checked once as its own `on_if` keyword and once as the
//!   previous branch's `else` -- a harmless duplicate, deduplicated by
//!   RuboCop's offense collection). Prism gives the terminal `else` its own
//!   real `ElseNode`, so this port checks `ElseNode::else_keyword_loc`
//!   directly (covering `if`/`unless`/`case`/`case/in`/`begin...rescue`
//!   alike, matching every upstream `:else` check without the duplicate).
//! - Only whitequark's real `:kwbegin` type (an explicit `begin...end`)
//!   reaches `on_kwbegin`; a method body implicitly wrapped for
//!   `rescue`/`ensure` is typed `:rescue`/`:ensure` directly, never
//!   `:kwbegin`. Prism's `BeginNode` covers both shapes, only distinguished
//!   by `begin_keyword_loc` being `None` for the implicit case, so this port
//!   skips `BeginNode` entirely when that location is absent (its
//!   `end_keyword_loc`, shared with the enclosing `def`, is never checked
//!   either).
//! - Whitequark's `if` node has no `:begin`/`:else`/`:keyword` methods at all
//!   for a ternary's `Ternary` location map, so `on_if` silently checks
//!   nothing; Prism's ternary `IfNode` has `if_keyword_loc: None`, used here
//!   as the equivalent early-out.
//! - `not foo` is whitequark's `(send foo :!)` with `loc.selector` reading
//!   `"not"`; Prism gives the identical shape as a `CallNode` with
//!   `receiver`, `name() == "!"`, and `message_loc` reading `"not"`.
//! - `preceded_by_operator?` climbs ancestors testing `operator_method?`
//!   (the method name of an ancestor `send` node) -- Prism's ancestor
//!   bookkeeping (`Context::ancestors`) only carries kind and span, not a
//!   method name, so this rule additionally subscribes to `CallNode` purely
//!   to record `span -> method name` as each is entered, then looks up an
//!   ancestor's name by its recorded span when climbing.

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `ACCEPT_LEFT_PAREN`: keywords for which a following `(` is not "missing
/// space", e.g. `break(1)`, `super(1)`.
const ACCEPT_LEFT_PAREN: &[&[u8]] =
    &[b"break", b"defined?", b"next", b"not", b"rescue", b"super", b"yield"];

/// `ACCEPT_LEFT_SQUARE_BRACKET`: keywords for which a following `[` is not
/// "missing space", e.g. `super[1]`, `yield[1]`.
const ACCEPT_LEFT_SQUARE_BRACKET: &[&[u8]] = &[b"super", b"yield"];

/// `ACCEPT_NAMESPACE_OPERATOR`: the one keyword for which a following `::`
/// is not "missing space" (`super::ModuleName`).
const ACCEPT_NAMESPACE_OPERATOR: &[u8] = b"super";

/// RuboCop-AST's `MethodIdentifierPredicates::OPERATOR_METHODS`, used by
/// `preceded_by_operator?`'s `ancestor.operator_method?`. Copied privately
/// from `layout/space_around_operators.rs`'s `is_operator_method_name`.
fn is_operator_method_name(name: &[u8]) -> bool {
    matches!(
        name,
        b"|" | b"^"
            | b"&"
            | b"<=>"
            | b"=="
            | b"==="
            | b"=~"
            | b">"
            | b">="
            | b"<"
            | b"<="
            | b"<<"
            | b">>"
            | b"+"
            | b"-"
            | b"*"
            | b"/"
            | b"%"
            | b"**"
            | b"~"
            | b"+@"
            | b"-@"
            | b"!@"
            | b"~@"
            | b"[]"
            | b"[]="
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

/// Use a space around keywords if appropriate.
#[derive(Debug, Clone, Default)]
pub struct SpaceAroundKeyword {
    /// `CallNode` span -> method name, recorded as each is entered, so a
    /// later ancestor climb can test `operator_method?` without re-parsing.
    /// See the module doc for why.
    call_names: HashMap<Span, Vec<u8>>,
}

impl Rule for SpaceAroundKeyword {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceAroundKeyword",
        department: Department::Layout,
        summary: "Use a space around keywords if appropriate.",
        explanation: "Checks the spacing around the keywords.\n\n\
            # bad\n\
            something 'test'do|x|\n\
            end\n\n\
            while(something)\n\
            end\n\n\
            something = 123if test\n\n\
            return(foo + bar)\n\n\
            # good\n\
            something 'test' do |x|\n\
            end\n\n\
            while (something)\n\
            end\n\n\
            something = 123 if test\n\n\
            return (foo + bar)",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::AndNode,
            NodeKind::BeginNode,
            NodeKind::BlockNode,
            NodeKind::BreakNode,
            NodeKind::CallNode,
            NodeKind::CaseNode,
            NodeKind::CaseMatchNode,
            NodeKind::DefinedNode,
            NodeKind::ElseNode,
            NodeKind::EnsureNode,
            NodeKind::ForNode,
            NodeKind::ForwardingSuperNode,
            NodeKind::IfNode,
            NodeKind::InNode,
            NodeKind::MatchPredicateNode,
            NodeKind::NextNode,
            NodeKind::OrNode,
            NodeKind::PostExecutionNode,
            NodeKind::PreExecutionNode,
            NodeKind::RescueModifierNode,
            NodeKind::RescueNode,
            NodeKind::ReturnNode,
            NodeKind::SuperNode,
            NodeKind::UnlessNode,
            NodeKind::UntilNode,
            NodeKind::WhenNode,
            NodeKind::WhileNode,
            NodeKind::YieldNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    #[allow(clippy::too_many_lines)]
    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => self.on_call(node, ctx),
            NodeKind::AndNode => {
                let n = node.as_and_node().expect("kind matched");
                if ctx.text(n.operator_loc().span()) == b"and" {
                    self.check_keyword(ctx, n.operator_loc().span());
                }
            }
            NodeKind::OrNode => {
                let n = node.as_or_node().expect("kind matched");
                if ctx.text(n.operator_loc().span()) == b"or" {
                    self.check_keyword(ctx, n.operator_loc().span());
                }
            }
            NodeKind::BeginNode => {
                let n = node.as_begin_node().expect("kind matched");
                let Some(begin_kw) = n.begin_keyword_loc() else { return };
                self.check_keyword(ctx, begin_kw.span());
                if let Some(end_kw) = n.end_keyword_loc() {
                    Self::check_before_only(ctx, end_kw.span());
                }
            }
            NodeKind::BlockNode => {
                let n = node.as_block_node().expect("kind matched");
                let do_form = ctx.text(n.opening_loc().span()) == b"do";
                if do_form {
                    self.check_keyword(ctx, n.opening_loc().span());
                    Self::check_before_only(ctx, n.closing_loc().span());
                }
            }
            NodeKind::BreakNode => {
                let n = node.as_break_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
            }
            NodeKind::CaseNode => {
                let n = node.as_case_node().expect("kind matched");
                self.check_keyword(ctx, n.case_keyword_loc().span());
            }
            NodeKind::CaseMatchNode => {
                let n = node.as_case_match_node().expect("kind matched");
                self.check_keyword(ctx, n.case_keyword_loc().span());
            }
            NodeKind::DefinedNode => {
                let n = node.as_defined_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
            }
            NodeKind::ElseNode => {
                let n = node.as_else_node().expect("kind matched");
                self.check_keyword(ctx, n.else_keyword_loc().span());
            }
            NodeKind::EnsureNode => {
                let n = node.as_ensure_node().expect("kind matched");
                self.check_keyword(ctx, n.ensure_keyword_loc().span());
            }
            NodeKind::ForNode => {
                let n = node.as_for_node().expect("kind matched");
                if let Some(do_kw) = n.do_keyword_loc() {
                    self.check_keyword(ctx, do_kw.span());
                    Self::check_before_only(ctx, n.end_keyword_loc().span());
                }
            }
            NodeKind::ForwardingSuperNode => {
                // Prism's `ForwardingSuperNode` has no `keyword_loc`; its
                // span covers the trailing block too (`super{}`), so the
                // keyword itself is always the first 5 bytes ("super").
                self.check_keyword(ctx, Span::new(node.span().start, node.span().start + 5));
            }
            NodeKind::RescueModifierNode => {
                let n = node.as_rescue_modifier_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
            }
            NodeKind::IfNode => {
                let n = node.as_if_node().expect("kind matched");
                let Some(if_kw) = n.if_keyword_loc() else { return };
                self.check_keyword(ctx, if_kw.span());
                if let Some(then_kw) = n.then_keyword_loc() {
                    if ctx.text(then_kw.span()) == b"then" {
                        self.check_keyword(ctx, then_kw.span());
                    }
                }
                if ctx.text(if_kw.span()) != b"elsif" {
                    if let Some(end_kw) = n.end_keyword_loc() {
                        Self::check_before_only(ctx, end_kw.span());
                    }
                }
            }
            NodeKind::InNode => {
                let n = node.as_in_node().expect("kind matched");
                self.check_keyword(ctx, n.in_loc().span());
            }
            NodeKind::MatchPredicateNode => {
                let n = node.as_match_predicate_node().expect("kind matched");
                self.check_keyword(ctx, n.operator_loc().span());
            }
            NodeKind::NextNode => {
                let n = node.as_next_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
            }
            NodeKind::PostExecutionNode => {
                let n = node.as_post_execution_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
            }
            NodeKind::PreExecutionNode => {
                let n = node.as_pre_execution_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
            }
            NodeKind::RescueNode => {
                let n = node.as_rescue_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
            }
            NodeKind::ReturnNode => {
                let n = node.as_return_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
            }
            NodeKind::SuperNode => {
                let n = node.as_super_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
            }
            NodeKind::UnlessNode => {
                let n = node.as_unless_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
                if let Some(then_kw) = n.then_keyword_loc() {
                    if ctx.text(then_kw.span()) == b"then" {
                        self.check_keyword(ctx, then_kw.span());
                    }
                }
                if let Some(end_kw) = n.end_keyword_loc() {
                    Self::check_before_only(ctx, end_kw.span());
                }
            }
            NodeKind::UntilNode => {
                let n = node.as_until_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
                if let Some(do_kw) = n.do_keyword_loc() {
                    self.check_keyword(ctx, do_kw.span());
                    if let Some(closing) = n.closing_loc() {
                        Self::check_before_only(ctx, closing.span());
                    }
                }
            }
            NodeKind::WhenNode => {
                let n = node.as_when_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
            }
            NodeKind::WhileNode => {
                let n = node.as_while_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
                if let Some(do_kw) = n.do_keyword_loc() {
                    self.check_keyword(ctx, do_kw.span());
                    if let Some(closing) = n.closing_loc() {
                        Self::check_before_only(ctx, closing.span());
                    }
                }
            }
            NodeKind::YieldNode => {
                let n = node.as_yield_node().expect("kind matched");
                self.check_keyword(ctx, n.keyword_loc().span());
            }
            _ => {}
        }
    }
}

impl SpaceAroundKeyword {
    /// `on_send`: records every call's method name (for later ancestor
    /// lookups by `preceded_by_operator?`) and checks `not foo`'s `"not"`
    /// selector, RuboCop-AST's `prefix_not?`.
    fn on_call(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        self.call_names.insert(node.span(), call.name().as_slice().to_vec());
        if call.receiver().is_some() && call.name().as_slice() == b"!" {
            if let Some(message_loc) = call.message_loc() {
                if ctx.text(message_loc.span()) == b"not" {
                    self.check_keyword(ctx, message_loc.span());
                }
            }
        }
    }

    /// RuboCop's `check_keyword`: both the "space before" (guarded by
    /// `preceded_by_operator?`) and "space after" checks.
    fn check_keyword(&self, ctx: &mut Context<'_>, span: Span) {
        if Self::space_before_missing(ctx, span) && !self.preceded_by_operator(ctx) {
            let msg = format!(
                "Space before keyword `{}` is missing.",
                String::from_utf8_lossy(ctx.text(span))
            );
            ctx.report_with_fix(
                &Self::META,
                span,
                msg,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::insert(span.start, b" ".to_vec())],
                },
            );
        }
        if Self::space_after_missing(ctx, span) {
            let msg = format!(
                "Space after keyword `{}` is missing.",
                String::from_utf8_lossy(ctx.text(span))
            );
            ctx.report_with_fix(
                &Self::META,
                span,
                msg,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::insert(span.end, b" ".to_vec())],
                },
            );
        }
    }

    /// RuboCop's `check_end`: only the "space before" half, unconditionally
    /// (this rule's callers already applied the `do?`/`begin_keyword`
    /// gating that upstream's `check_end` performs before calling it).
    fn check_before_only(ctx: &mut Context<'_>, span: Span) {
        if Self::space_before_missing(ctx, span) {
            let msg = format!(
                "Space before keyword `{}` is missing.",
                String::from_utf8_lossy(ctx.text(span))
            );
            ctx.report_with_fix(
                &Self::META,
                span,
                msg,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::insert(span.start, b" ".to_vec())],
                },
            );
        }
    }

    /// RuboCop's `space_before_missing?`.
    fn space_before_missing(ctx: &Context<'_>, span: Span) -> bool {
        if span.start == 0 {
            return false;
        }
        let before = ctx.text(Span::new(span.start - 1, span.start))[0];
        !matches!(
            before,
            b' ' | b'\t' | b'\n' | b'\r' | b'(' | b'|' | b'{' | b'[' | b';' | b',' | b'*' | b'='
        )
    }

    /// RuboCop's `space_after_missing?`.
    fn space_after_missing(ctx: &Context<'_>, span: Span) -> bool {
        let keyword = ctx.text(span);
        let source = ctx.source().bytes();
        let pos = span.end as usize;
        let Some(&ch) = source.get(pos) else { return false };
        if (ACCEPT_LEFT_SQUARE_BRACKET.contains(&keyword) && ch == b'[')
            || (ACCEPT_LEFT_PAREN.contains(&keyword) && ch == b'(')
        {
            return false;
        }
        if source[pos..].starts_with(b"&.") {
            return false;
        }
        if keyword == ACCEPT_NAMESPACE_OPERATOR && source[pos..].starts_with(b"::") {
            return false;
        }
        !matches!(
            ch,
            b' ' | b'\t' | b'\n' | b'\r' | b';' | b',' | b'#' | b'\\' | b')' | b'}' | b']' | b'.'
        )
    }

    /// RuboCop's `preceded_by_operator?`: climbs ancestors from the
    /// immediate parent outward, accepting an `and`/`or` node or a range
    /// literal unconditionally, an operator-named `CallNode` ancestor
    /// (looked up by span in `self.call_names`, see the module doc), and
    /// otherwise stopping at the first non-`CallNode` ancestor.
    ///
    /// `ArgumentsNode` is transparent: whitequark's `send` node holds its
    /// arguments directly as children with no wrapper, so `each_ancestor`
    /// steps straight from an argument to the enclosing `send`; Prism
    /// interposes an `ArgumentsNode`, which this climb skips over rather
    /// than treating as a climb-stopping non-`CallNode` ancestor.
    fn preceded_by_operator(&self, ctx: &Context<'_>) -> bool {
        for ancestor in ctx.ancestors().iter().rev() {
            match ancestor.kind {
                NodeKind::AndNode | NodeKind::OrNode | NodeKind::RangeNode => return true,
                NodeKind::ArgumentsNode => {}
                NodeKind::CallNode => {
                    if self
                        .call_names
                        .get(&ancestor.span)
                        .is_some_and(|n| is_operator_method_name(n))
                    {
                        return true;
                    }
                }
                _ => return false,
            }
        }
        false
    }
}

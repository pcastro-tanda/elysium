//! `Style/EndlessMethod`, ported from RuboCop's
//! `lib/rubocop/cop/style/endless_method.rb` plus the
//! `EndlessMethodRewriter` mixin it includes for autocorrection.
//!
//! `node.parent&.any_def_type? && node.parent.endless?` (`endless_parent?`,
//! guarding against correcting a `def` that is itself nested as the sole
//! body expression of an enclosing endless `def`, e.g. `def a = def b = 1`)
//! relies on whitequark eliding a single-statement `begin`: the inner
//! `def`'s immediate parent there really is the outer `def` node. Prism
//! always wraps a body in a `StatementsNode`, so [`endless_parent`] looks
//! one level further up past a single-statement `StatementsNode` wrapper,
//! and consults `self.endless_spans` (populated for every endless `def`
//! already visited by this same pre-order walk, which -- since `enter`
//! fires parent-before-child -- always includes an enclosing `def` by the
//! time its nested body `def` is reached).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{DefNode, ParametersNode};
use ruby_ast::{each_descendant, ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::collections::HashSet;

const MSG: &str = "Avoid endless method definitions.";
const MSG_MULTI_LINE: &str = "Avoid endless method definitions with multiple lines.";
const MSG_REQUIRE_SINGLE: &str = "Use endless method definitions for single line methods.";
const MSG_REQUIRE_ALWAYS: &str = "Use endless method definitions.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    AllowSingleLine,
    AllowAlways,
    Disallow,
    RequireSingleLine,
    RequireAlways,
}

/// Checks for endless methods.
#[derive(Debug, Clone)]
pub struct EndlessMethod {
    style: Style,
    /// `Alignment#configured_indentation_width`: this cop has no
    /// `IndentationWidth` option of its own, so it always reads
    /// `Layout/IndentationWidth`'s `Width` (default 2).
    indentation_width: i64,
    /// `Layout/LineLength`'s configured `Max`, or `None` when that cop is
    /// disabled -- `too_long_when_made_endless?`'s `config.cop_enabled?` guard.
    max_line_length: Option<i64>,
    /// `TargetRubyVersion` mixin's `minimum_target_ruby_version 3.0`: below
    /// it, endless methods are not valid syntax and the whole cop is a
    /// no-op.
    target_ruby_version: f32,
    /// Spans of every endless `def` node seen so far by this same
    /// pre-order walk (see the module doc's `endless_parent` note).
    endless_spans: HashSet<Span>,
}

impl Rule for EndlessMethod {
    const META: RuleMeta = RuleMeta {
        name: "Style/EndlessMethod",
        department: Department::Style,
        summary: "Avoid the use of multi-lined endless method definitions.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[linter::ConfigOption {
            name: "EnforcedStyle",
            default: linter::ConfigDefault::Str("allow_single_line"),
            allowed: &[
                "allow_single_line",
                "allow_always",
                "disallow",
                "require_single_line",
                "require_always",
            ],
            doc: "Whether/when endless method definitions are required, allowed, or disallowed.",
        }],
        blind_spots: "Reads `Layout/IndentationWidth`'s `Width` and `Layout/LineLength`'s \
                      `Max`/`Enabled` as peer options, matching upstream's own cross-cop reads.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "allow_always" => Style::AllowAlways,
            "disallow" => Style::Disallow,
            "require_single_line" => Style::RequireSingleLine,
            "require_always" => Style::RequireAlways,
            _ => Style::AllowSingleLine,
        };
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        let max_line_length = options
            .peer("Layout/LineLength", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(true)
            .then(|| {
                options
                    .peer("Layout/LineLength", "Max")
                    .and_then(OptionValue::as_int)
                    .unwrap_or(120)
            });
        Ok(Self {
            style,
            indentation_width,
            max_line_length,
            target_ruby_version: options.target_ruby_version(),
            endless_spans: HashSet::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.target_ruby_version < 3.0 {
            return;
        }
        let def = node.as_def_node().expect("kind matched");
        let endless = def.equal_loc().is_some();
        if endless {
            self.endless_spans.insert(node.span());
        }
        if is_assignment_method(def.name().as_slice()) || use_heredoc(def.body()) {
            return;
        }
        match self.style {
            Style::AllowSingleLine | Style::AllowAlways => {
                self.handle_allow_style(&def, node, endless, ctx);
            }
            Style::Disallow => self.handle_disallow_style(&def, node, endless, ctx),
            Style::RequireSingleLine => {
                self.handle_require_single_line_style(&def, node, endless, ctx);
            }
            Style::RequireAlways => self.handle_require_always_style(&def, node, endless, ctx),
        }
    }
}

impl EndlessMethod {
    fn handle_allow_style(
        &self,
        def: &DefNode<'_>,
        node: &Node<'_>,
        endless: bool,
        ctx: &mut Context<'_>,
    ) {
        if !endless {
            return;
        }
        if ctx.is_single_line(node.span()) || self.style == Style::AllowAlways {
            return;
        }
        self.report_multiline(def, node, MSG_MULTI_LINE, ctx);
    }

    fn handle_disallow_style(
        &self,
        def: &DefNode<'_>,
        node: &Node<'_>,
        endless: bool,
        ctx: &mut Context<'_>,
    ) {
        if !endless {
            return;
        }
        self.report_multiline(def, node, MSG, ctx);
    }

    fn handle_require_single_line_style(
        &self,
        def: &DefNode<'_>,
        node: &Node<'_>,
        endless: bool,
        ctx: &mut Context<'_>,
    ) {
        if endless && !ctx.is_single_line(node.span()) {
            self.report_multiline(def, node, MSG_MULTI_LINE, ctx);
        } else if !endless && can_be_made_endless(def.body()) {
            let replacement = endless_replacement(def, ctx);
            if replacement.contains('\n') {
                return;
            }
            if self.too_long(def, &replacement, ctx) {
                return;
            }
            report_endless(def, node, MSG_REQUIRE_SINGLE, ctx);
        }
    }

    fn handle_require_always_style(
        &self,
        def: &DefNode<'_>,
        node: &Node<'_>,
        endless: bool,
        ctx: &mut Context<'_>,
    ) {
        if endless || !can_be_made_endless(def.body()) {
            return;
        }
        let replacement = endless_replacement(def, ctx);
        if self.too_long(def, &replacement, ctx) {
            return;
        }
        report_endless(def, node, MSG_REQUIRE_ALWAYS, ctx);
    }

    fn too_long(&self, def: &DefNode<'_>, replacement: &str, ctx: &Context<'_>) -> bool {
        let Some(max) = self.max_line_length else { return false };
        let column = i64::from(ctx.line_col(def.location().span().start).column);
        i64::try_from(replacement.len()).unwrap_or(i64::MAX) + column > max
    }

    fn report_multiline(
        &self,
        def: &DefNode<'_>,
        node: &Node<'_>,
        message: &'static str,
        ctx: &mut Context<'_>,
    ) {
        if endless_parent(node, &self.endless_spans, ctx) {
            ctx.report(&Self::META, node.span(), message);
            return;
        }
        let replacement = multiline_replacement(def, node, ctx, self.indentation_width);
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
            },
        );
    }
}

fn report_endless(
    def: &DefNode<'_>,
    node: &Node<'_>,
    message: &'static str,
    ctx: &mut Context<'_>,
) {
    let Some(body) = def.body() else { return };
    let Some(end_keyword) = def.end_keyword_loc() else { return };
    let signature_end = signature_end(def);
    let body_start = body.span().start;
    let body_end = body.span().end;
    ctx.report_with_fix(
        &EndlessMethod::META,
        node.span(),
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![
                Edit::replace(Span::new(signature_end, body_start), b" = ".to_vec()),
                Edit::delete(Span::new(body_end, end_keyword.span().end)),
            ],
        },
    );
}

/// RuboCop-AST's `assignment_method?`: a method name ending in `=` that is
/// not one of the comparison operators.
fn is_assignment_method(name: &[u8]) -> bool {
    name.ends_with(b"=") && !matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=")
}

/// RuboCop's `use_heredoc?`: the body is itself a heredoc string, or
/// contains one anywhere within it.
fn use_heredoc(body: Option<Node<'_>>) -> bool {
    let Some(body) = body else { return false };
    if ext::is_heredoc(&body) {
        return true;
    }
    let mut found = false;
    each_descendant(&body, &mut |n| {
        if ext::is_heredoc(n) {
            found = true;
        }
    });
    found
}

/// RuboCop's `can_be_made_endless?`: the body must not be a multi-statement
/// sequence, nor a `begin`/`rescue`/`ensure` wrapper -- Prism always wraps a
/// body in a `StatementsNode`, so a single-statement one (whitequark's
/// elided-`begin` case) is the only shape left standing besides a bare
/// `BeginNode` (an explicit or implicit `rescue`/`ensure`, never elided).
fn can_be_made_endless(body: Option<Node<'_>>) -> bool {
    match body {
        None => false,
        Some(b) => match b.kind() {
            NodeKind::BeginNode => false,
            NodeKind::StatementsNode => {
                let Some(s) = b.as_statements_node() else { return false };
                let items = s.body();
                items.len() == 1
                    && items.first().is_some_and(|only| only.kind() != NodeKind::BeginNode)
            }
            _ => true,
        },
    }
}

/// `node.parent&.any_def_type? && node.parent.endless?`, adjusted for
/// Prism's always-present single-statement `StatementsNode` wrapper (see
/// the module doc).
fn endless_parent(node: &Node<'_>, endless_spans: &HashSet<Span>, ctx: &Context<'_>) -> bool {
    let ancestors = ctx.ancestors();
    let Some(immediate) = ancestors.last() else { return false };
    let target = if immediate.kind == NodeKind::StatementsNode {
        ancestors.get(ancestors.len() - 2)
    } else {
        Some(immediate)
    };
    let _ = node;
    target.is_some_and(|info| info.kind == NodeKind::DefNode && endless_spans.contains(&info.span))
}

/// RuboCop's `EndlessMethodRewriter#receiver`.
fn receiver_text(def: &DefNode<'_>, ctx: &Context<'_>) -> String {
    match (def.receiver(), def.operator_loc()) {
        (Some(receiver), Some(operator)) => {
            format!(
                "{}{}",
                String::from_utf8_lossy(ctx.text(receiver.span())),
                String::from_utf8_lossy(ctx.text(operator.span()))
            )
        }
        _ => String::new(),
    }
}

/// `arguments.any?`, false for an empty `()` or no parameter list at all.
fn params_present(params: &ParametersNode<'_>) -> bool {
    !params.requireds().is_empty()
        || !params.optionals().is_empty()
        || params.rest().is_some()
        || !params.posts().is_empty()
        || !params.keywords().is_empty()
        || params.keyword_rest().is_some()
        || params.block().is_some()
}

/// RuboCop's `EndlessMethodRewriter#arguments`: the parameter list's own
/// source, parens included when present, else `missing` (`''` for the
/// endless-signature text, `()` padding never needed since whitequark's
/// args-node source already covers the parens itself).
fn arguments_text(def: &DefNode<'_>, ctx: &Context<'_>) -> String {
    match def.parameters() {
        Some(params) if params_present(&params) => {
            let span = match (def.lparen_loc(), def.rparen_loc()) {
                (Some(lparen), Some(rparen)) => Span::new(lparen.span().start, rparen.span().end),
                _ => params.location().span(),
            };
            String::from_utf8_lossy(ctx.text(span)).into_owned()
        }
        _ => String::new(),
    }
}

/// RuboCop's `EndlessMethodRewriter#endless_replacement`: the single-line
/// text an endless rewrite of `node` would produce, used only to measure
/// length/multilinearity before actually committing to the (differently
/// implemented) `correct_to_endless` edits.
fn endless_replacement(def: &DefNode<'_>, ctx: &Context<'_>) -> String {
    let Some(body) = def.body() else { return String::new() };
    format!(
        "def {}{}{} = {}",
        receiver_text(def, ctx),
        String::from_utf8_lossy(ctx.text(def.name_loc().span())),
        arguments_text(def, ctx),
        String::from_utf8_lossy(ctx.text(body.span())),
    )
}

/// RuboCop's `EndlessMethodRewriter#signature_to_body_range`'s start: past
/// the parameter list when there is one, else past the method name.
fn signature_end(def: &DefNode<'_>) -> u32 {
    match def.parameters() {
        Some(params) if params_present(&params) => {
            def.rparen_loc().map_or(params.location().span().end, |r| r.span().end)
        }
        _ => def.name_loc().span().end,
    }
}

/// RuboCop's `EndlessMethodRewriter#correct_to_multiline`.
fn multiline_replacement(
    def: &DefNode<'_>,
    node: &Node<'_>,
    ctx: &Context<'_>,
    indentation_width: i64,
) -> String {
    let Some(body) = def.body() else {
        return String::from_utf8_lossy(ctx.text(node.span())).into_owned();
    };
    let column = i64::from(ctx.line_col(node.span().start).column);
    let body_indent = " ".repeat(usize::try_from((column + indentation_width).max(0)).unwrap_or(0));
    let end_indent = " ".repeat(usize::try_from(column.max(0)).unwrap_or(0));
    format!(
        "def {}{}{}\n{body_indent}{}\n{end_indent}end",
        receiver_text(def, ctx),
        String::from_utf8_lossy(ctx.text(def.name_loc().span())),
        arguments_text(def, ctx),
        String::from_utf8_lossy(ctx.text(body.span())),
    )
}

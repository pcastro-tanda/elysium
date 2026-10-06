//! `Minitest/GlobalExpectations`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/global_expectations.rb` (with the matcher
//! lists of `MinitestExplorationHelpers`).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const VALUE_MATCHERS: &[&[u8]] = &[
    b"must_be_empty",
    b"must_equal",
    b"must_be_close_to",
    b"must_be_within_delta",
    b"must_be_within_epsilon",
    b"must_include",
    b"must_be_instance_of",
    b"must_be_kind_of",
    b"must_match",
    b"must_be_nil",
    b"must_be",
    b"must_respond_to",
    b"must_be_same_as",
    b"path_must_exist",
    b"path_wont_exist",
    b"wont_be_empty",
    b"wont_equal",
    b"wont_be_close_to",
    b"wont_be_within_delta",
    b"wont_be_within_epsilon",
    b"wont_include",
    b"wont_be_instance_of",
    b"wont_be_kind_of",
    b"wont_match",
    b"wont_be_nil",
    b"wont_be",
    b"wont_respond_to",
    b"wont_be_same_as",
];

const BLOCK_MATCHERS: &[&[u8]] = &[
    b"must_output",
    b"must_pattern_match",
    b"must_raise",
    b"must_be_silent",
    b"must_throw",
    b"wont_pattern_match",
];

/// There are aliases for the `_` method - `expect` and `value`.
const DSL_METHODS: &[&[u8]] = &[b"_", b"expect", b"value"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Underscore,
    Any,
    Expect,
    Value,
}

/// This cop checks for deprecated global expectations and autocorrects them
/// to use expect format.
#[derive(Debug, Clone)]
pub struct GlobalExpectations {
    style: Style,
}

impl Rule for GlobalExpectations {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/GlobalExpectations",
        department: Department::Minitest,
        summary: "This cop checks for deprecated global expectations.",
        explanation: "Checks for deprecated global expectations and autocorrects them to use \
                      expect format.\n\n```ruby\n# EnforcedStyle: any (default)\n# bad\n\
                      musts.must_equal expected_musts\nwonts.wont_match expected_wonts\n\
                      musts.must_raise TypeError\n\n# good\n\
                      _(musts).must_equal expected_musts\n_(wonts).wont_match expected_wonts\n\
                      _ { musts }.must_raise TypeError\n\n\
                      expect(musts).must_equal expected_musts\n\
                      expect(wonts).wont_match expected_wonts\n\
                      expect { musts }.must_raise TypeError\n\n\
                      value(musts).must_equal expected_musts\n\
                      value(wonts).wont_match expected_wonts\n\
                      value { musts }.must_raise TypeError\n```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::CallOperatorWriteNode,
            NodeKind::CallOrWriteNode,
            NodeKind::CallAndWriteNode,
        ],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("any"),
            allowed: &["_", "any", "expect", "value"],
            doc: "Which spelling of the expectation DSL receiver to enforce.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "_" => Style::Underscore,
            "expect" => Style::Expect,
            "value" => Style::Value,
            _ => Style::Any,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        // `on_send` fires for `send` nodes only (not `csend`), including the
        // getter `send` inside `a.b += 1`, `a.b ||= 1` and `a.b &&= 1`.
        let (receiver, name) = if let Some(call) = node.as_call_node() {
            if call.is_safe_navigation() {
                return;
            }
            (call.receiver(), call.name())
        } else if let Some(write) = node.as_call_operator_write_node() {
            if write.is_safe_navigation() {
                return;
            }
            (write.receiver(), write.read_name())
        } else if let Some(write) = node.as_call_or_write_node() {
            if write.is_safe_navigation() {
                return;
            }
            (write.receiver(), write.read_name())
        } else if let Some(write) = node.as_call_and_write_node() {
            if write.is_safe_navigation() {
                return;
            }
            (write.receiver(), write.read_name())
        } else {
            return;
        };
        let name = name.as_slice();
        let is_block_matcher = BLOCK_MATCHERS.contains(&name);
        if !is_block_matcher && !VALUE_MATCHERS.contains(&name) {
            return;
        }
        let Some(receiver) = receiver else { return };

        let method = block_receiver(&receiver).or_else(|| value_receiver(&receiver));
        let preferred = self.preferred_method();
        if method == Some(preferred) || (method.is_some() && self.style == Style::Any) {
            return;
        }

        let receiver_span = receiver.span();
        let receiver_source = String::from_utf8_lossy(ctx.text(receiver_span)).into_owned();
        let (preferred_text, replacement) = if let Some(method) = method {
            // `receiver.source.sub(method.to_s, preferred_method.to_s)`
            let replacement = receiver_source.replacen(method, preferred, 1);
            (preferred.to_owned(), replacement)
        } else {
            let Some(preferred_receiver) =
                self.preferred_receiver(&receiver, is_block_matcher, ctx)
            else {
                return;
            };
            (preferred_receiver.clone(), preferred_receiver)
        };

        ctx.report_with_fix(
            &Self::META,
            receiver_span,
            format!("Use `{preferred_text}` instead."),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(receiver_span, replacement.into_bytes())],
            },
        );
    }
}

impl GlobalExpectations {
    fn preferred_method(&self) -> &'static str {
        match self.style {
            Style::Any | Style::Underscore => "_",
            Style::Expect => "expect",
            Style::Value => "value",
        }
    }

    fn preferred_receiver(
        &self,
        receiver: &Node<'_>,
        is_block_matcher: bool,
        ctx: &Context<'_>,
    ) -> Option<String> {
        let preferred = self.preferred_method();
        if is_block_matcher {
            let source = match lambda_body(receiver) {
                // `receiver.lambda? ? receiver.body : receiver`; an empty
                // lambda body has no `source` upstream (NoMethodError).
                LambdaBody::Body(body) => ctx.text(body).to_vec(),
                LambdaBody::Empty => return None,
                LambdaBody::NotLambda => ctx.text(receiver.span()).to_vec(),
            };
            Some(format!("{preferred} {{ {} }}", String::from_utf8_lossy(&source)))
        } else {
            Some(format!("{preferred}({})", String::from_utf8_lossy(ctx.text(receiver.span()))))
        }
    }
}

/// `(block (send nil? $#method_allowed?) _ _)`.
fn block_receiver(receiver: &Node<'_>) -> Option<&'static str> {
    let call = receiver.as_call_node()?;
    let block = call.block()?;
    let block = block.as_block_node()?;
    // `numblock`/`itblock` are other node types.
    if let Some(parameters) = block.parameters() {
        if parameters.as_numbered_parameters_node().is_some()
            || parameters.as_it_parameters_node().is_some()
        {
            return None;
        }
    }
    if call.receiver().is_some() || call.is_safe_navigation() || call.arguments().is_some() {
        return None;
    }
    dsl_method(&call)
}

/// `(send nil? $#method_allowed? _)`.
fn value_receiver(receiver: &Node<'_>) -> Option<&'static str> {
    let call = receiver.as_call_node()?;
    if call.receiver().is_some() || call.is_safe_navigation() {
        return None;
    }
    // A `BlockNode` makes it a `block`/`numblock`; `&blk` is an argument.
    let mut count = 0;
    if let Some(arguments) = call.arguments() {
        count += arguments.arguments().iter().count();
    }
    if let Some(block) = call.block() {
        block.as_block_argument_node()?;
        count += 1;
    }
    if count != 1 {
        return None;
    }
    dsl_method(&call)
}

fn dsl_method(call: &CallNode<'_>) -> Option<&'static str> {
    let name = call.name();
    match name.as_slice() {
        b"_" => Some("_"),
        b"expect" => Some("expect"),
        b"value" => Some("value"),
        other => {
            debug_assert!(!DSL_METHODS.contains(&other));
            None
        }
    }
}

enum LambdaBody {
    NotLambda,
    Empty,
    Body(Span),
}

/// `Node#lambda?` (`(any_block (send nil? :lambda) ...)`) followed by
/// `receiver.body`.
fn lambda_body(receiver: &Node<'_>) -> LambdaBody {
    let body = if let Some(lambda) = receiver.as_lambda_node() {
        lambda.body()
    } else {
        let Some(call) = receiver.as_call_node() else { return LambdaBody::NotLambda };
        if call.receiver().is_some()
            || call.name().as_slice() != b"lambda"
            || call.is_safe_navigation()
            || call.arguments().is_some()
        {
            return LambdaBody::NotLambda;
        }
        let Some(block) = call.block().and_then(|block| block.as_block_node()) else {
            return LambdaBody::NotLambda;
        };
        block.body()
    };
    body.map_or(LambdaBody::Empty, |body| LambdaBody::Body(body_span(&body)))
}

/// The source range of a block body the way whitequark sizes it: the
/// statements without the closing `end` of a `rescue`/`ensure` body.
fn body_span(body: &Node<'_>) -> Span {
    let span = body.span();
    if let Some(begin) = body.as_begin_node() {
        if let Some(end) = begin.end_keyword_loc() {
            return Span::new(span.start, end.span().start);
        }
    }
    span
}

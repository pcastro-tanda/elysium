//! `Rails/ContentTag`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/content_tag.rb`.
//!
//! RuboCop-AST's `each_ancestor(:send)` walks `send` ancestors only (not
//! `csend`), and a block's call is *not* an ancestor of the block body. Prism
//! nests the body inside the call, so the rule keeps its own stack of
//! enclosing calls and skips those whose block contains the node asked about.

use super::rails_version::target_rails_version;
use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `minimum_target_rails_version 5.1`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.1;

/// One enclosing call on the stack.
#[derive(Debug, Clone, Copy)]
struct Enclosing {
    /// `csend`: not matched by `each_ancestor(:send)`.
    safe_navigation: bool,
    /// The call's literal block, which holds nodes that are not its operands.
    block: Option<Span>,
    /// `@corrected_nodes.include?`.
    corrected: bool,
}

/// Use `tag.something` instead of `tag(:something)`.
#[derive(Debug, Clone)]
pub struct ContentTag {
    supported: bool,
    stack: Vec<Enclosing>,
    allowed_name: Regex,
    upper_run: Regex,
    lower_upper: Regex,
}

impl Rule for ContentTag {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ContentTag",
        department: Department::Rails,
        summary: "Use `tag.something` instead of `tag(:something)`.",
        explanation: "Use `tag` instead of `content_tag` or `tag` with a name argument: \
                      `tag.something` instead of `tag(:something)`.\n\n```ruby\n# bad\n\
                      tag(:p, 'Hello world!')\ntag(:br, class: 'strong')\n\n# good\n\
                      tag.p('Hello world!')\ntag.br(class: 'strong')\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            supported: target_rails_version(options) >= MINIMUM_TARGET_RAILS_VERSION,
            stack: Vec::new(),
            // Ruby's `^`/`$` match at every line.
            allowed_name: Regex::new(r"(?m)^[a-zA-Z-][a-zA-Z\-0-9]*$").expect("static regex"),
            upper_run: Regex::new(r"([A-Z0-9]+)([A-Z][a-z])").expect("static regex"),
            lower_upper: Regex::new(r"([a-z0-9])([A-Z])").expect("static regex"),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.stack.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let corrected = self.check(&call, node.span(), ctx);
        self.stack.push(Enclosing {
            safe_navigation: call.is_safe_navigation(),
            block: block_span(&call),
            corrected,
        });
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if self.supported && node.as_call_node().is_some() {
            self.stack.pop();
        }
    }
}

fn block_span(call: &CallNode<'_>) -> Option<Span> {
    let block = call.block()?.as_block_node()?.location();
    Some(block.span())
}

impl ContentTag {
    /// `on_send`; whether an offense was registered.
    fn check(&self, call: &CallNode<'_>, span: Span, ctx: &mut Context<'_>) -> bool {
        if call.name().as_slice() != b"tag" || call.receiver().is_some() {
            return false;
        }
        let mut arguments: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        // `&block` is one more argument for RuboCop.
        if let Some(block) = call.block().filter(|b| b.as_block_argument_node().is_some()) {
            arguments.push(block);
        }
        if arguments.len() >= 3 {
            return false;
        }
        let Some(first) = arguments.first() else { return false };
        let Some(value) = self.value_of(first, ctx) else { return false };
        if self.corrected_ancestor(span) {
            return false;
        }

        let preferred_method = self.underscore(&value);
        let first_source = String::from_utf8_lossy(ctx.text(first.span())).into_owned();
        let message = format!("Use `tag.{preferred_method}` instead of `tag({first_source})`.");

        let node_span = call_span_excluding_block(call);
        let Some(selector) = call.message_loc() else { return false };
        let range = Span::new(selector.span().start, node_span.end);
        let rest = arguments[1..]
            .iter()
            .map(|argument| String::from_utf8_lossy(ctx.text(argument.span())).into_owned())
            .collect::<Vec<_>>()
            .join(", ");
        let replacement = format!("tag.{preferred_method}({rest})");
        ctx.report_with_fix(
            &Self::META,
            node_span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, replacement.into_bytes())],
            },
        );
        true
    }

    /// `node.each_ancestor(:send).any? { |a| @corrected_nodes&.include?(a) }`.
    fn corrected_ancestor(&self, span: Span) -> bool {
        self.stack.iter().any(|enclosing| {
            let inside_block = enclosing
                .block
                .is_some_and(|block| block.start <= span.start && span.end <= block.end);
            enclosing.corrected && !enclosing.safe_navigation && !inside_block
        })
    }

    /// `argument.value` of an argument that is not `allowed_argument?`
    /// (variables, calls, constants, splats and anything without `value`
    /// are allowed, hence `None`).
    fn value_of(&self, argument: &Node<'_>, ctx: &Context<'_>) -> Option<String> {
        if let Some(string) = argument.as_string_node() {
            let value = String::from_utf8_lossy(string.unescaped()).into_owned();
            return self.allowed_name.is_match(&value).then_some(value);
        }
        if let Some(symbol) = argument.as_symbol_node() {
            let value = String::from_utf8_lossy(symbol.unescaped()).into_owned();
            return self.allowed_name.is_match(&value).then_some(value);
        }
        if let Some(interpolated) = argument.as_interpolated_string_node() {
            // `DstrNode#value`: the parts' values, interpolations as source.
            let mut value = String::new();
            for part in &interpolated.parts() {
                if let Some(string) = part.as_string_node() {
                    value.push_str(&String::from_utf8_lossy(string.unescaped()));
                } else {
                    value.push_str(&String::from_utf8_lossy(ctx.text(part.span())));
                }
            }
            return Some(value);
        }
        if argument.as_integer_node().is_some()
            || argument.as_float_node().is_some()
            || argument.as_rational_node().is_some()
            || argument.as_imaginary_node().is_some()
        {
            // `Integer#to_s` and friends, approximated by the literal's source.
            return Some(String::from_utf8_lossy(ctx.text(argument.span())).into_owned());
        }
        None
    }

    /// `ActiveSupport`'s `String#underscore`.
    fn underscore(&self, word: &str) -> String {
        let word = self.upper_run.replace_all(word, "${1}_${2}");
        let word = self.lower_upper.replace_all(&word, "${1}_${2}");
        word.replace('-', "_").to_lowercase()
    }
}

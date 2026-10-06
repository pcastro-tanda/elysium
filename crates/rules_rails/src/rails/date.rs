//! `Rails/Date`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/date.rb`.
//!
//! RuboCop-AST's `each_ancestor(:send)` walks `send` ancestors only (not
//! `csend`), and a block's call is *not* an ancestor of the block body. Prism
//! nests the body inside the call, so the rule keeps its own stack of
//! enclosing calls and skips those whose block contains the node asked about.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::bytes::Regex;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_SEND: &str =
    "Do not use `{}` on Date objects, because they know nothing about the time zone in use.";

/// Every name the rule asks about, so a chain can be kept as small indices:
/// `BAD_DAYS` (0..4), `bad_methods` (4..6), then `TimeZone::ACCEPTED_METHODS`.
const NAMES: [&str; 16] = [
    "today",
    "current",
    "yesterday",
    "tomorrow",
    "to_time",
    "to_time_in_current_zone",
    "in_time_zone",
    "utc",
    "getlocal",
    "xmlschema",
    "iso8601",
    "jisx0301",
    "rfc3339",
    "httpdate",
    "to_i",
    "to_f",
];

fn name_index(name: &[u8]) -> Option<u8> {
    NAMES.iter().position(|n| n.as_bytes() == name).and_then(|i| u8::try_from(i).ok())
}

/// One enclosing call on the stack.
#[derive(Debug, Clone, Copy)]
struct Enclosing {
    /// `NAMES` index, `None` for any other method.
    name: Option<u8>,
    /// `csend`: not matched by `each_ancestor(:send)`.
    safe_navigation: bool,
    /// The call's literal block, which holds nodes that are not its operands.
    block: Option<Span>,
}

/// Checks the correct usage of date aware methods, such as Date.today, Date.current etc.
#[derive(Debug, Clone)]
pub struct Date {
    strict: bool,
    allow_to_time: bool,
    stack: Vec<Enclosing>,
    zone: Regex,
}

impl Rule for Date {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Date",
        department: Department::Rails,
        summary:
            "Checks the correct usage of date aware methods, such as Date.today, Date.current etc.",
        explanation: "Checks for the correct use of Date methods, such as Date.today, \
                      Date.current etc.\n\nUsing `Date.today` is dangerous, because it doesn't \
                      know anything about Rails time zone. You must use `Time.zone.today` \
                      instead.\n\nThe cop also reports warnings when you are using `to_time` \
                      method, because it doesn't know about Rails time zone either.\n\nTwo \
                      styles are supported for this cop. When `EnforcedStyle` is `strict` then \
                      the Date methods `today`, `current`, `yesterday`, and `tomorrow` are \
                      prohibited and the usage of both `to_time` and \
                      `to_time_in_current_zone` are reported as warning.\n\nWhen \
                      `EnforcedStyle` is `flexible` then only `Date.today` is prohibited.\n\n\
                      And you can set a warning for `to_time` with `AllowToTime: false`. \
                      `AllowToTime` is `true` by default to prevent false positive on \
                      `DateTime` object.\n\nThis cop's autocorrection is unsafe because it may \
                      change handling time.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("flexible"),
                allowed: &["strict", "flexible"],
                doc: "`strict` also disallows `Date.current` and friends and `to_time`.",
            },
            ConfigOption {
                name: "AllowToTime",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether `to_time` is allowed.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            strict: options.style("EnforcedStyle")? == "strict",
            allow_to_time: options.bool("AllowToTime"),
            stack: Vec::new(),
            // `[+-][\d:]+|\dZ` at the very end (`\z`).
            zone: Regex::new(r"(?:[+-][0-9:]+|[0-9]Z)\z").expect("static regex"),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.stack.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let span = node.span();

        if let Some(receiver) = call.receiver() {
            if !call.is_safe_navigation() && is_core_date(&receiver) {
                self.check_date_node(&call, span, ctx);
            }
            self.check_to_time(&call, &receiver, span, ctx);
        }

        let block = call.block().and_then(|block| block.as_block_node().map(|b| b.location()));
        self.stack.push(Enclosing {
            name: name_index(call.name().as_slice()),
            safe_navigation: call.is_safe_navigation(),
            block: block.map(|loc| loc.span()),
        });
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_call_node().is_some() {
            self.stack.pop();
        }
    }
}

impl Date {
    /// `[node, *node.each_ancestor(:send)].map(&:method_name)`, as indices
    /// into `NAMES` (`None` for unrelated names), starting from the node.
    fn chain(&self, own: Option<u8>, span: Span) -> impl Iterator<Item = Option<u8>> + '_ {
        std::iter::once(own).chain(self.stack.iter().rev().filter_map(move |enclosing| {
            let inside_block = enclosing
                .block
                .is_some_and(|block| block.start <= span.start && span.end <= block.end);
            (!enclosing.safe_navigation && !inside_block).then_some(enclosing.name)
        }))
    }

    fn is_bad_day(&self, index: u8) -> bool {
        // `bad_days`: BAD_DAYS minus the good ones (all but `today` when flexible).
        index < 4 && (self.strict || index == 0)
    }

    fn is_good_method(&self, index: u8) -> bool {
        // `good_methods`: ACCEPTED_METHODS unless strict.
        !self.strict && index >= 6
    }

    fn check_date_node(
        &self,
        call: &ruby_ast::node::CallNode<'_>,
        span: Span,
        ctx: &mut Context<'_>,
    ) {
        let Some(selector) = call.message_loc() else { return };
        let own = name_index(call.name().as_slice());

        let mut days: Vec<u8> = Vec::new();
        for index in self.chain(own, span).flatten() {
            if self.is_bad_day(index) && !days.contains(&index) {
                days.push(index);
            }
        }
        if days.is_empty() {
            return;
        }
        let method_name = days.iter().map(|&i| NAMES[usize::from(i)]).collect::<Vec<_>>().join(".");
        let day = if method_name == "current" { "today" } else { method_name.as_str() };
        let message =
            format!("Do not use `Date.{method_name}` without zone. Use `Time.zone.{day}` instead.");

        let Some(receiver) = call.receiver() else { return };
        let name_span = if let Some(path) = receiver.as_constant_path_node() {
            let loc = path.name_loc();
            loc.span()
        } else {
            receiver.span()
        };
        ctx.report_with_fix(
            &Self::META,
            selector.span(),
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(name_span, b"Time.zone".to_vec())],
            },
        );
    }

    fn check_to_time(
        &self,
        call: &ruby_ast::node::CallNode<'_>,
        receiver: &Node<'_>,
        span: Span,
        ctx: &mut Context<'_>,
    ) {
        let name = call.name();
        let name = name.as_slice();
        if !matches!(name, b"to_time" | b"to_time_in_current_zone") {
            return;
        }
        let is_to_time = name == b"to_time";
        if self.allow_to_time && is_to_time {
            return;
        }
        let own = name_index(name);
        // `safe_chain?`: the chain always holds the node's own bad method, so
        // only a good method anywhere in it makes it safe.
        if self.chain(own, span).flatten().any(|index| self.is_good_method(index)) {
            return;
        }
        if is_to_time && self.safe_to_time(call, receiver) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let selector = selector.span();

        if name == b"to_time_in_current_zone" {
            ctx.report_with_fix(
                &Self::META,
                selector,
                "`to_time_in_current_zone` is deprecated. Use `in_time_zone` instead.",
                Fix {
                    applicability: Applicability::Unsafe,
                    edits: vec![Edit::replace(selector, b"in_time_zone".to_vec())],
                },
            );
            // RuboCop drops the second offense at an already-reported range.
            return;
        }
        let method = String::from_utf8_lossy(name);
        ctx.report(&Self::META, selector, MSG_SEND.replace("{}", &method));
    }

    /// `safe_to_time?`: a zone-suffixed string literal, or a zone argument.
    fn safe_to_time(&self, call: &ruby_ast::node::CallNode<'_>, receiver: &Node<'_>) -> bool {
        if let Some(string) = receiver.as_string_node() {
            return self.zone.is_match(string.unescaped());
        }
        let arguments = call.arguments().map_or(0, |a| a.arguments().iter().count());
        let block_pass = call.block().is_some_and(|b| b.as_block_argument_node().is_some());
        arguments + usize::from(block_pass) == 1
    }
}

/// `(const {nil? cbase} :Date)`.
fn is_core_date(node: &Node<'_>) -> bool {
    if let Some(read) = node.as_constant_read_node() {
        return read.name().as_slice() == b"Date";
    }
    node.as_constant_path_node().is_some_and(|path| {
        path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"Date")
    })
}

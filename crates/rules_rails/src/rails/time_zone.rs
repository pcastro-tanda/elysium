//! `Rails/TimeZone`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/time_zone.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_LOCALTIME: &str = "Do not use `Time.localtime` without offset or zone.";
const MSG_STRING_TO_TIME: &str =
    "Do not use `String#to_time` without zone. Use `Time.zone.parse` instead.";

const GOOD_METHODS: [&str; 4] = ["zone", "zone_default", "find_zone", "find_zone!"];
const DANGEROUS_METHODS: [&str; 5] = ["now", "local", "new", "parse", "at"];
const ACCEPTED_METHODS: [&str; 10] = [
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

/// Checks for the use of Time methods without zone.
#[derive(Debug, Clone)]
pub struct TimeZone {
    /// `EnforcedStyle: strict`; otherwise `flexible`.
    strict: bool,
}

impl Rule for TimeZone {
    const META: RuleMeta = RuleMeta {
        name: "Rails/TimeZone",
        department: Department::Rails,
        summary: "Checks the correct usage of time zone aware methods.",
        explanation: "Checks for the use of Time methods without zone.\n\nBuilt on top of Ruby \
                      on Rails style guide (https://rails.rubystyle.guide#time)\nand the article \
                      http://danilenko.org/2012/7/6/rails_timezones/\n\nTwo styles are supported \
                      for this cop. When `EnforcedStyle` is 'strict'\nthen only use of \
                      `Time.zone` is allowed.\n\nWhen EnforcedStyle is 'flexible' then it's also \
                      allowed\nto use `Time#in_time_zone`.\n\nThis cop's autocorrection is unsafe \
                      because it may change handling time.\n\n```ruby\n# bad\nTime.now\n\
                      Time.parse('2015-03-02T19:05:37')\n'2015-03-02T19:05:37'.to_time\n\n# \
                      good\nTime.current\nTime.zone.now\nTime.zone.parse('2015-03-02T19:05:37')\n\
                      Time.zone.parse('2015-03-02T19:05:37Z') # Respect ISO 8601 format with \
                      timezone specifier.\nTime.parse('2015-03-02T19:05:37Z') # Also respects \
                      ISO 8601\n'2015-03-02T19:05:37Z'.to_time # Also respects ISO 8601\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("flexible"),
            allowed: &["strict", "flexible"],
            doc: "`strict` means that `Time` should be used with `zone`; `flexible` also \
                  allows `in_time_zone`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { strict: options.style("EnforcedStyle")? == "strict" })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // `on_send` / `on_csend` for `String#to_time`.
        if call.name().as_slice() == b"to_time" {
            Self::check_string_to_time(&call, ctx);
        }
        // `on_const` for a `Time` / `::Time` that is the receiver of a `send`.
        if call.is_safe_navigation() {
            return;
        }
        if call.receiver().is_some_and(|receiver| is_time_const(&receiver)) {
            self.check_time_node(&call, ctx);
        }
    }
}

/// `(const {nil? cbase} :Time)`.
fn is_time_const(node: &Node<'_>) -> bool {
    if let Some(constant) = node.as_constant_read_node() {
        return constant.name().as_slice() == b"Time";
    }
    node.as_constant_path_node().is_some_and(|path| {
        path.parent().is_none() && path.name().is_some_and(|name| name.as_slice() == b"Time")
    })
}

/// Owned facts about a `send` ancestor, enough for the method chain walk.
#[derive(Debug)]
struct CallInfo {
    name: String,
    /// `method_from_time_class?`.
    time_class: bool,
    has_arguments: bool,
    has_literal_block: bool,
    selector: Option<Span>,
}

/// One entry of a node's ancestor stack.
#[derive(Debug)]
enum Ancestor {
    /// Prism's `ArgumentsNode` (not a node in whitequark).
    Arguments,
    /// A `send` (never `csend`).
    Call(CallInfo),
    Other,
}

impl TimeZone {
    fn check_string_to_time(call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let Some(receiver) = call.receiver() else { return };
        if receiver.as_string_node().is_none() || attach_timezone_specifier(&receiver) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let selector = selector.span();
        if call.is_safe_navigation() {
            ctx.report(&Self::META, selector, MSG_STRING_TO_TIME);
            return;
        }
        let span = call_span_excluding_block(call);
        let replacement = format!("Time.zone.parse({})", lossy(ctx.text(receiver.span())));
        ctx.report_with_fix(
            &Self::META,
            selector,
            MSG_STRING_TO_TIME,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            },
        );
    }

    fn check_time_node(&self, node: &CallNode<'_>, ctx: &mut Context<'_>) {
        let first_argument =
            node.arguments().and_then(|arguments| arguments.arguments().iter().next());
        if first_argument.as_ref().is_some_and(attach_timezone_specifier) {
            return;
        }
        let calls = send_chain(ctx, node);
        let chain: Vec<&str> =
            calls.iter().filter(|call| call.time_class).map(|call| call.name.as_str()).collect();

        if self.not_danger_chain(&chain) {
            return;
        }
        let Some(selector) = node.message_loc().map(|loc| loc.span()) else { return };
        if !self.strict && chain.contains(&"localtime") {
            self.check_localtime(node, &calls, &chain, ctx, selector);
            return;
        }
        let method_name = dangerous_in(&chain).join(".");
        if offset_provided(node) {
            return;
        }
        let message = self.build_message(&method_name, node);
        self.report(ctx, selector, message, node, &calls, &chain);
    }

    fn check_localtime(
        &self,
        node: &CallNode<'_>,
        calls: &[CallInfo],
        chain: &[&str],
        ctx: &mut Context<'_>,
        selector: Span,
    ) {
        let Some(localtime) = calls.iter().find(|call| call.name == "localtime") else { return };
        if localtime.has_arguments {
            return;
        }
        self.report(ctx, selector, MSG_LOCALTIME.to_owned(), node, calls, chain);
    }

    fn report(
        &self,
        ctx: &mut Context<'_>,
        selector: Span,
        message: String,
        node: &CallNode<'_>,
        calls: &[CallInfo],
        chain: &[&str],
    ) {
        let edits = self.autocorrect(node, calls, chain, ctx);
        ctx.report_with_fix(
            &Self::META,
            selector,
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }

    fn autocorrect(
        &self,
        node: &CallNode<'_>,
        calls: &[CallInfo],
        chain: &[&str],
        ctx: &Context<'_>,
    ) -> Vec<Edit> {
        let mut edits = Vec::new();
        // Add `.zone` (`Time.at` => `Time.zone.at`); strict style also
        // prefers `Time` over `::Time`.
        if let Some(receiver) = node.receiver() {
            let span = receiver.span();
            let base = if self.strict { "Time".to_owned() } else { lossy(ctx.text(span)) };
            edits.push(Edit::replace(span, format!("{base}.zone").into_bytes()));
        }
        if let Some(selector) = node.message_loc().map(|loc| loc.span()) {
            match node.name().as_slice() {
                b"current" => edits.push(Edit::replace(selector, b"now".to_vec())),
                b"new" => {
                    edits.push(Edit::replace(selector, replacement(node).as_bytes().to_vec()));
                }
                _ => {}
            }
        }
        // Remove redundant `.in_time_zone` from `Time.zone.now.in_time_zone`.
        if chain.contains(&"in_time_zone") || chain.contains(&"zone") {
            for call in calls {
                if call.name == "in_time_zone" && !call.has_arguments {
                    if let Some(selector) = call.selector {
                        edits.push(Edit::delete(Span::new(selector.start - 1, selector.end)));
                    }
                }
            }
        }
        edits
    }

    fn not_danger_chain(&self, chain: &[&str]) -> bool {
        dangerous_in(chain).is_empty() || chain.iter().any(|name| self.is_good_method(name))
    }

    fn is_good_method(&self, name: &str) -> bool {
        GOOD_METHODS.contains(&name)
            || (!self.strict && (name == "current" || ACCEPTED_METHODS.contains(&name)))
    }

    fn build_message(&self, method_name: &str, node: &CallNode<'_>) -> String {
        let safe = safe_method(method_name, node);
        if self.strict {
            format!("Do not use `Time.{method_name}` without zone. Use `Time.zone.{safe}` instead.")
        } else {
            let mut acceptable = vec![format!("`Time.zone.{safe}`"), "`Time.current`".to_owned()];
            for accepted in ACCEPTED_METHODS {
                acceptable.push(format!("`Time.{method_name}.{accepted}`"));
            }
            format!(
                "Do not use `Time.{method_name}` without zone. Use one of {} instead.",
                acceptable.join(", ")
            )
        }
    }
}

/// `chain & DANGEROUS_METHODS`: the chain's dangerous methods, in chain
/// order, without duplicates.
fn dangerous_in<'a>(chain: &[&'a str]) -> Vec<&'a str> {
    let mut found: Vec<&str> = Vec::new();
    for name in chain {
        if DANGEROUS_METHODS.contains(name) && !found.contains(name) {
            found.push(name);
        }
    }
    found
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn safe_method(method_name: &str, node: &CallNode<'_>) -> String {
    if method_name == "new" || method_name == "current" {
        replacement(node).to_owned()
    } else {
        method_name.to_owned()
    }
}

/// `replacement(node)`.
fn replacement(node: &CallNode<'_>) -> &'static str {
    match node.arguments().and_then(|arguments| arguments.arguments().iter().next()) {
        None => {
            if node.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
                "local"
            } else {
                "now"
            }
        }
        Some(first) if first.as_string_node().is_some() => "parse",
        Some(_) => "local",
    }
}

/// `offset_provided?`.
fn offset_provided(node: &CallNode<'_>) -> bool {
    match node.name().as_slice() {
        b"new" => argument_count(node) == 7 || offset_option_provided(node),
        b"at" | b"now" => offset_option_provided(node),
        _ => false,
    }
}

/// `node.arguments.size`, counting a `&block` argument as whitequark does.
fn argument_count(node: &CallNode<'_>) -> usize {
    let positional = node.arguments().map_or(0, |arguments| arguments.arguments().iter().count());
    positional + usize::from(node.block().is_some_and(|b| b.as_block_argument_node().is_some()))
}

/// `offset_option_provided?`: a trailing hash with a non-nil `in:` pair.
fn offset_option_provided(node: &CallNode<'_>) -> bool {
    if node.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
        return false;
    }
    let Some(arguments) = node.arguments() else { return false };
    let Some(last) = arguments.arguments().iter().last() else { return false };
    let elements = if let Some(hash) = last.as_hash_node() {
        hash.elements()
    } else if let Some(hash) = last.as_keyword_hash_node() {
        hash.elements()
    } else {
        return false;
    };
    elements.iter().any(|element| {
        element.as_assoc_node().is_some_and(|pair| {
            pair.key().as_symbol_node().is_some_and(|key| key.unescaped() == b"in")
                && pair.value().as_nil_node().is_none()
        })
    })
}

/// `attach_timezone_specifier?`: a literal with a `value` whose text ends in
/// a letter or a numeric UTC offset (or has an invalid encoding).
fn attach_timezone_specifier(date: &Node<'_>) -> bool {
    let bytes: Vec<u8> = if let Some(string) = date.as_string_node() {
        string.unescaped().to_vec()
    } else if let Some(interpolated) = date.as_interpolated_string_node() {
        // `DstrNode#value`: literal parts' values joined with the source of the rest.
        let mut joined = Vec::new();
        for part in interpolated.parts().iter() {
            match part.as_string_node() {
                Some(string) => joined.extend_from_slice(string.unescaped()),
                None => joined.extend_from_slice(part.location().as_slice()),
            }
        }
        joined
    } else if let Some(symbol) = date.as_symbol_node() {
        symbol.unescaped().to_vec()
    } else if let Some(integer) = date.as_integer_node() {
        if !integer.is_decimal() {
            return false;
        }
        let text = lossy(date.location().as_slice());
        text.replace('_', "").into_bytes()
    } else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else { return true };
    has_timezone_specifier(text.as_bytes())
}

/// `TIMEZONE_SPECIFIER = /([A-Za-z]|[+-]\d{2}:?\d{2})\z/`.
fn has_timezone_specifier(text: &[u8]) -> bool {
    let n = text.len();
    if text.last().is_some_and(u8::is_ascii_alphabetic) {
        return true;
    }
    let sign = |i: usize| matches!(text[i], b'+' | b'-');
    let digits = |from: usize, to: usize| text[from..to].iter().all(u8::is_ascii_digit);
    (n >= 5 && sign(n - 5) && digits(n - 4, n))
        || (n >= 6
            && sign(n - 6)
            && digits(n - 5, n - 3)
            && text[n - 3] == b':'
            && digits(n - 2, n))
}

/// `method_from_time_class?`: does the receiver chain bottom out at `Time`?
fn method_from_time_class(node: &Node<'_>) -> bool {
    if let Some(call) = node.as_call_node() {
        return call_from_time_class(&call);
    }
    if let Some(constant) = node.as_constant_read_node() {
        return constant.name().as_slice() == b"Time";
    }
    if let Some(path) = node.as_constant_path_node() {
        return match path.parent() {
            Some(parent) => method_from_time_class(&parent),
            None => path.name().is_some_and(|name| name.as_slice() == b"Time"),
        };
    }
    false
}

fn call_from_time_class(call: &CallNode<'_>) -> bool {
    match call.receiver() {
        Some(receiver) => method_from_time_class(&receiver),
        None => call.name().as_slice() == b"Time",
    }
}

fn call_info(call: &CallNode<'_>) -> CallInfo {
    let block = call.block();
    CallInfo {
        name: lossy(call.name().as_slice()),
        time_class: call_from_time_class(call),
        has_arguments: call.arguments().is_some()
            || block.as_ref().is_some_and(|b| b.as_block_argument_node().is_some()),
        has_literal_block: block.as_ref().is_some_and(|b| b.as_block_node().is_some()),
        selector: call.message_loc().map(|loc| loc.span()),
    }
}

fn ancestor_info(node: &Node<'_>) -> Ancestor {
    if node.as_arguments_node().is_some() {
        Ancestor::Arguments
    } else if let Some(call) = node.as_call_node() {
        if call.is_safe_navigation() {
            Ancestor::Other
        } else {
            Ancestor::Call(call_info(&call))
        }
    } else {
        Ancestor::Other
    }
}

/// Finds the node `target` (of `kind`) below `node`, leaving its ancestors,
/// outermost first, in `path`.
fn find_ancestors(node: &Node<'_>, target: Span, kind: NodeKind, path: &mut Vec<Ancestor>) -> bool {
    let span = node.span();
    if span == target && node.kind() == kind {
        return true;
    }
    // No end-of-span pruning: a heredoc's body lies outside its parents' spans.
    if span.start > target.start {
        return false;
    }
    path.push(ancestor_info(node));
    let mut found = false;
    for_each_child(node, |child| {
        if !found {
            found = find_ancestors(child, target, kind, path);
        }
    });
    if !found {
        path.pop();
    }
    found
}

/// The `send` nodes `extract_method_chain` walks: the node itself, then each
/// `send` parent, stopping at a parent that is not a plain `send` and after a
/// `send` that has a literal block (its parent is the `block`).
fn send_chain(ctx: &Context<'_>, node: &CallNode<'_>) -> Vec<CallInfo> {
    let own = call_info(node);
    let mut chain = Vec::new();
    let mut stop = own.has_literal_block;
    chain.push(own);
    if stop {
        return chain;
    }
    let mut path = Vec::new();
    let root = ctx.parsed().root();
    let target = node.location().span();
    find_ancestors(&root, target, NodeKind::CallNode, &mut path);
    for ancestor in path.into_iter().rev() {
        if stop {
            break;
        }
        match ancestor {
            Ancestor::Arguments => {}
            Ancestor::Call(info) => {
                stop = info.has_literal_block;
                chain.push(info);
            }
            Ancestor::Other => break,
        }
    }
    chain
}

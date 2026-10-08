//! `Rails/HttpStatus`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/http_status.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// `Rack::Utils::SYMBOL_TO_STATUS_CODE` (rack 3.2.7), in insertion order.
const SYMBOL_TO_STATUS_CODE: &[(&str, i32)] = &[
    ("continue", 100),
    ("switching_protocols", 101),
    ("processing", 102),
    ("early_hints", 103),
    ("ok", 200),
    ("created", 201),
    ("accepted", 202),
    ("non_authoritative_information", 203),
    ("no_content", 204),
    ("reset_content", 205),
    ("partial_content", 206),
    ("multi_status", 207),
    ("already_reported", 208),
    ("im_used", 226),
    ("multiple_choices", 300),
    ("moved_permanently", 301),
    ("found", 302),
    ("see_other", 303),
    ("not_modified", 304),
    ("use_proxy", 305),
    ("temporary_redirect", 307),
    ("permanent_redirect", 308),
    ("bad_request", 400),
    ("unauthorized", 401),
    ("payment_required", 402),
    ("forbidden", 403),
    ("not_found", 404),
    ("method_not_allowed", 405),
    ("not_acceptable", 406),
    ("proxy_authentication_required", 407),
    ("request_timeout", 408),
    ("conflict", 409),
    ("gone", 410),
    ("length_required", 411),
    ("precondition_failed", 412),
    ("content_too_large", 413),
    ("uri_too_long", 414),
    ("unsupported_media_type", 415),
    ("range_not_satisfiable", 416),
    ("expectation_failed", 417),
    ("misdirected_request", 421),
    ("unprocessable_content", 422),
    ("locked", 423),
    ("failed_dependency", 424),
    ("too_early", 425),
    ("upgrade_required", 426),
    ("precondition_required", 428),
    ("too_many_requests", 429),
    ("request_header_fields_too_large", 431),
    ("unavailable_for_legal_reasons", 451),
    ("internal_server_error", 500),
    ("not_implemented", 501),
    ("bad_gateway", 502),
    ("service_unavailable", 503),
    ("gateway_timeout", 504),
    ("http_version_not_supported", 505),
    ("variant_also_negotiates", 506),
    ("insufficient_storage", 507),
    ("loop_detected", 508),
    ("network_authentication_required", 511),
];

/// `NumericStyleChecker::PERMITTED_STATUS`.
const PERMITTED_STATUS: &[&str] = &["error", "success", "missing", "redirect"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Symbolic,
    Numeric,
}

/// Enforces use of symbolic or numeric value to define HTTP status.
#[derive(Debug, Clone)]
pub struct HttpStatus {
    style: Style,
}

impl Rule for HttpStatus {
    const META: RuleMeta = RuleMeta {
        name: "Rails/HttpStatus",
        department: Department::Rails,
        summary: "Enforces use of symbolic or numeric value to define HTTP status.",
        explanation: "Enforces use of symbolic or numeric value to define HTTP status.\n\n\
                      ```ruby\n# EnforcedStyle: symbolic (default)\n# bad\nrender :foo, status: \
                      200\nhead 200\n\n# good\nrender :foo, status: :ok\nhead :ok\n\n\
                      # EnforcedStyle: numeric\n# bad\nrender :foo, status: :ok\nhead :ok\n\n\
                      # good\nrender :foo, status: 200\nhead 200\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("symbolic"),
            allowed: &["numeric", "symbolic"],
            doc: "Whether to prefer symbolic or numeric HTTP statuses.",
        }],
        blind_spots:
            "The status table is `Rack::Utils::SYMBOL_TO_STATUS_CODE` of rack 3.2.7; \
                      other rack versions differ for a few statuses (e.g. `:unprocessable_entity`).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = if options.style("EnforcedStyle")? == "numeric" {
            Style::Numeric
        } else {
            Style::Symbolic
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_some() {
            return;
        }
        let name = call.name();
        let name = name.as_slice();
        if !matches!(
            name,
            b"render" | b"redirect_to" | b"head" | b"assert_response" | b"assert_redirected_to"
        ) {
            return;
        }
        // A `&blk` argument is one more `send` argument in whitequark.
        let mut arguments: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        let has_block_arg = call.block().is_some_and(|b| b.as_block_argument_node().is_some());
        if has_block_arg {
            if let Some(block) = call.block() {
                arguments.push(block);
            }
        }
        let is_hash =
            |n: &Node<'_>| n.as_hash_node().is_some() || n.as_keyword_hash_node().is_some();
        let status = match name {
            b"render" | b"redirect_to" => match arguments.as_slice() {
                [first] if is_hash(first) => status_code(first),
                [_, second] if is_hash(second) => status_code(second),
                _ => None,
            },
            b"head" | b"assert_response" => match arguments.first() {
                Some(first)
                    if first.as_integer_node().is_some() || first.as_symbol_node().is_some() =>
                {
                    Some(*first)
                }
                _ => None,
            },
            _ => match arguments.as_slice() {
                [first, ..] if is_hash(first) => status_code(first),
                [_, second, ..] if is_hash(second) => status_code(second),
                _ => None,
            },
        };
        let Some(status) = status else { return };
        let Some((message, preferred)) = self.check(&status) else { return };
        let span = status.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, preferred.into_bytes())],
            },
        );
    }
}

impl HttpStatus {
    /// The checker's `offensive?` / `message` / `preferred_style`.
    fn check(&self, node: &Node<'_>) -> Option<(String, String)> {
        match self.style {
            Style::Symbolic => {
                if node.as_symbol_node().is_some() {
                    return None;
                }
                let (number, current) = if let Some(int) = node.as_integer_node() {
                    let number = TryInto::<i32>::try_into(int.value()).ok()?;
                    // `custom_http_status_code?`.
                    if !SYMBOL_TO_STATUS_CODE.iter().any(|&(_, v)| v == number) {
                        return None;
                    }
                    (number, number.to_string())
                } else {
                    let text =
                        String::from_utf8_lossy(node.as_string_node()?.unescaped()).into_owned();
                    (ruby_to_i(&text), text)
                };
                let preferred = SYMBOL_TO_STATUS_CODE
                    .iter()
                    .find(|&&(_, v)| v == number)
                    .map_or_else(|| "nil".to_owned(), |&(k, _)| format!(":{k}"));
                Some((
                    format!("Prefer `{preferred}` over `{current}` to define HTTP status code."),
                    preferred,
                ))
            }
            Style::Numeric => {
                let symbol = node.as_symbol_node()?;
                let name = String::from_utf8_lossy(symbol.unescaped()).into_owned();
                if PERMITTED_STATUS.contains(&name.as_str()) {
                    return None;
                }
                let &(_, number) = SYMBOL_TO_STATUS_CODE.iter().find(|&&(k, _)| k == name)?;
                let preferred = number.to_string();
                Some((
                    format!("Prefer `{preferred}` over `:{name}` to define HTTP status code."),
                    preferred,
                ))
            }
        }
    }
}

/// `status_code`: the value of the first `status:` pair, when an integer,
/// symbol or string literal.
fn status_code<'pr>(hash: &Node<'pr>) -> Option<Node<'pr>> {
    let elements = match (hash.as_hash_node(), hash.as_keyword_hash_node()) {
        (Some(h), _) => h.elements(),
        (None, Some(k)) => k.elements(),
        _ => return None,
    };
    elements.iter().find_map(|element| {
        let pair = element.as_assoc_node()?;
        let key = pair.key().as_symbol_node()?.unescaped().to_vec();
        if key != b"status" {
            return None;
        }
        let value = pair.value();
        (value.as_integer_node().is_some()
            || value.as_symbol_node().is_some()
            || value.as_string_node().is_some())
        .then_some(value)
    })
}

/// `String#to_i`: optional whitespace and sign, then digits (with single
/// underscores between them); `0` otherwise. Saturates instead of
/// overflowing: such a number is in no status table.
fn ruby_to_i(text: &str) -> i32 {
    let text = text.trim_start();
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let mut value: i64 = 0;
    let mut previous_underscore = true;
    for byte in digits.bytes() {
        match byte {
            b'0'..=b'9' => {
                value = (value * 10 + i64::from(byte - b'0')).min(i64::from(i32::MAX));
                previous_underscore = false;
            }
            b'_' if !previous_underscore => previous_underscore = true,
            _ => break,
        }
    }
    let value = i32::try_from(value).unwrap_or(i32::MAX);
    if negative {
        -value
    } else {
        value
    }
}

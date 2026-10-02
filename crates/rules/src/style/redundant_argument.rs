//! `Style/RedundantArgument`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_argument.rb`.
//!
//! Upstream's `argument_matched?` always compares against
//! `redundant_argument.inspect`'s *rendered* text, and
//! `exclude_cntrl_character?` is a tautology once `redundant_argument` is
//! itself always sourced from `Object#inspect` (see the YAML-configured
//! `Methods` values): `inspect` output never contains a raw control byte, so
//! `!redundant_argument.match?(/[[:cntrl:]]/)` is always true and the
//! expression it feeds (`||`-chained) is always true regardless of the
//! target argument's own encoding or content. So this port always compares
//! against the literal-argument's own Ruby-`inspect` rendering (the
//! `target_argument.is_a?(AST::Node)` branch still falls back to the
//! argument's raw source for anything that is not a `String`/`Integer`/
//! `true`/`false` literal).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::{Side, Span};

/// Checks for a redundant argument passed to certain methods.
#[derive(Debug, Clone)]
pub struct RedundantArgument {
    methods: Vec<(String, OptionValue)>,
}

impl Rule for RedundantArgument {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantArgument",
        department: Department::Style,
        summary: "Checks for a redundant argument passed to certain methods.",
        explanation: "Checks for a redundant argument passed to certain methods. \n\n\
            NOTE: This cop is limited to methods with single parameter. \n\n\
            Method names and their redundant arguments can be configured via `Methods`. \n\n\
            @safety\n\
            This cop is unsafe because of the following limitations:\n\n\
            1. This cop matches by method names only and hence cannot tell apart \
            methods with same name in different classes.\n\
            2. This cop may be unsafe if certain special global variables (e.g. `$;`, `$/`) are \
            set. That depends on the nature of the target methods, of course. For example, the \
            default argument to join is `$OUTPUT_FIELD_SEPARATOR` (or `$,`) rather than `''`, \
            and if that global is changed, `''` is no longer a redundant argument.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let methods = options
            .get("Methods")
            .and_then(OptionValue::as_map)
            .map(<[_]>::to_vec)
            .unwrap_or_default();
        Ok(Self { methods })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name().as_slice();
        let no_receiver_method = name == b"exit" || name == b"exit!";
        if !no_receiver_method && call.receiver().is_none() {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let list = args.arguments();
        if list.len() != 1 {
            return;
        }
        let arg = list.first().expect("checked len");

        let Some((_, config_value)) =
            self.methods.iter().find(|(method_name, _)| method_name.as_bytes() == name)
        else {
            return;
        };

        if !argument_matches(ctx, &arg, config_value) {
            return;
        }

        let offense_range = argument_range(ctx, &call, &arg);
        let arg_source = String::from_utf8_lossy(ctx.text(arg.location().span())).into_owned();
        let message =
            format!("Argument {arg_source} is redundant because it is implied by default.");

        ctx.report_with_fix(
            &Self::META,
            offense_range,
            message,
            Fix { applicability: Applicability::Unsafe, edits: vec![Edit::delete(offense_range)] },
        );
    }
}

/// RuboCop's `redundant_argument?`/`argument_matched?`.
fn argument_matches(ctx: &Context<'_>, arg: &Node<'_>, redundant: &OptionValue) -> bool {
    let rendered = if let Some(string) = arg.as_string_node() {
        ruby_string_inspect(string.unescaped())
    } else if let Some(int) = arg.as_integer_node() {
        match TryInto::<i32>::try_into(int.value()) {
            Ok(value) => value.to_string(),
            Err(()) => return false,
        }
    } else if arg.as_true_node().is_some() {
        "true".to_string()
    } else if arg.as_false_node().is_some() {
        "false".to_string()
    } else {
        // Not a literal with a `.value`: upstream compares the argument's
        // own source text against `redundant_argument`.
        String::from_utf8_lossy(ctx.text(arg.location().span())).into_owned()
    };
    rendered == ruby_inspect_option_value(redundant)
}

/// `Object#inspect` for the scalar `OptionValue`s `Methods` entries can hold.
fn ruby_inspect_option_value(value: &OptionValue) -> String {
    match value {
        OptionValue::Str(s) => ruby_string_inspect(s.as_bytes()),
        OptionValue::Int(i) => i.to_string(),
        OptionValue::Bool(true) => "true".to_string(),
        OptionValue::Bool(false) => "false".to_string(),
        OptionValue::Float(f) => f.to_string(),
        OptionValue::Null | OptionValue::List(_) | OptionValue::Map(_) => String::new(),
    }
}

/// Ruby's `String#inspect`: double-quoted, with backslashes, double quotes,
/// and control/invalid bytes escaped.
fn ruby_string_inspect(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() + 2);
    out.push('"');
    match std::str::from_utf8(bytes) {
        Ok(s) => {
            for ch in s.chars() {
                push_inspected_char(&mut out, ch);
            }
        }
        Err(_) => {
            for &b in bytes {
                push_inspected_byte(&mut out, b);
            }
        }
    }
    out.push('"');
    out
}

fn push_inspected_char(out: &mut String, ch: char) {
    match ch {
        '"' => out.push_str("\\\""),
        '\\' => out.push_str("\\\\"),
        '\n' => out.push_str("\\n"),
        '\t' => out.push_str("\\t"),
        '\r' => out.push_str("\\r"),
        '\x1b' => out.push_str("\\e"),
        '\x07' => out.push_str("\\a"),
        '\x08' => out.push_str("\\b"),
        '\x0c' => out.push_str("\\f"),
        '\x0b' => out.push_str("\\v"),
        '\0' => out.push_str("\\0"),
        c if (c as u32) < 0x20 || (c as u32) == 0x7f || (c as u32) > 0xff => {
            let _ = std::fmt::Write::write_fmt(out, format_args!("\\x{:02X}", c as u32));
        }
        c => out.push(c),
    }
}

fn push_inspected_byte(out: &mut String, b: u8) {
    match b {
        b'"' => out.push_str("\\\""),
        b'\\' => out.push_str("\\\\"),
        b'\n' => out.push_str("\\n"),
        b'\t' => out.push_str("\\t"),
        b'\r' => out.push_str("\\r"),
        0x1b => out.push_str("\\e"),
        0x07 => out.push_str("\\a"),
        0x08 => out.push_str("\\b"),
        0x0c => out.push_str("\\f"),
        0x0b => out.push_str("\\v"),
        0x00 => out.push_str("\\0"),
        0x20..=0x7e => out.push(b as char),
        _ => {
            let _ = std::fmt::Write::write_fmt(out, format_args!("\\x{b:02X}"));
        }
    }
}

/// RuboCop's `argument_range`.
fn argument_range(ctx: &Context<'_>, call: &ruby_ast::node::CallNode<'_>, arg: &Node<'_>) -> Span {
    match (call.opening_loc(), call.closing_loc()) {
        (Some(open), Some(close)) => Span::new(open.span().start, close.span().end),
        _ => ctx.with_surrounding_space(arg.location().span(), Side::Both, false, false),
    }
}

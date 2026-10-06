//! `Rails/DotSeparatedKeys`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/dot_separated_keys.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use the dot-separated keys instead of specifying the `:scope` option.";

/// Enforces the use of dot-separated locale keys instead of specifying the
/// `:scope` option with an array or a single symbol in `I18n` translation
/// methods.
#[derive(Debug, Clone)]
pub struct DotSeparatedKeys;

impl Rule for DotSeparatedKeys {
    const META: RuleMeta = RuleMeta {
        name: "Rails/DotSeparatedKeys",
        department: Department::Rails,
        summary: "Enforces the use of dot-separated keys instead of `:scope` options in `I18n` translation methods.",
        explanation: "Enforces the use of dot-separated locale keys instead of specifying the \
                      `:scope` option with an array or a single symbol in `I18n` translation \
                      methods. Dot-separated notation is easier to read and trace the \
                      hierarchy.\n\n```ruby\n# bad\nI18n.t :record_invalid, scope: \
                      [:activerecord, :errors, :messages]\nI18n.t :title, scope: \
                      :invitation\n\n# good\nI18n.t 'activerecord.errors.messages.record_invalid'\n\
                      I18n.t :record_invalid, scope: 'activerecord.errors.messages'\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Scope elements that are `true`, `false`, `nil`, rationals, complex numbers or \
                      non-decimal integers are treated as non-literal (no offense).",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if !matches!(call.name().as_slice(), b"translate" | b"t") || call.is_safe_navigation() {
            return;
        }
        if let Some(receiver) = call.receiver() {
            if !is_bare_or_toplevel_const(&receiver)
                || const_name(&receiver).as_deref() != Some("I18n")
            {
                return;
            }
        }
        if call.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let list = arguments.arguments();
        let mut iter = list.iter();
        let (Some(key_node), Some(options), None) = (iter.next(), iter.next(), iter.next()) else {
            return;
        };
        let Some(key) = basic_value(ctx, &key_node, false) else { return };
        let elements = if let Some(hash) = options.as_keyword_hash_node() {
            hash.elements()
        } else if let Some(hash) = options.as_hash_node() {
            hash.elements()
        } else {
            return;
        };
        // `<$(pair (sym :scope) ${array_type? sym_type?}) ...>`: the first
        // matching pair.
        let Some((pair_span, scope_value)) = elements.iter().find_map(|element| {
            let pair = element.as_assoc_node()?;
            let name = pair.key().as_symbol_node()?;
            if name.unescaped() != b"scope" {
                return None;
            }
            let value = pair.value();
            (value.as_array_node().is_some() || value.as_symbol_node().is_some())
                .then(|| (pair.as_node().span(), value))
        }) else {
            return;
        };
        let scopes: Vec<Node<'_>> = match scope_value.as_array_node() {
            Some(array) => array.elements().iter().collect(),
            None => vec![scope_value],
        };
        let mut parts = Vec::with_capacity(scopes.len());
        for scope in &scopes {
            let Some(text) = basic_value(ctx, scope, true) else { return };
            parts.push(text);
        }
        let new_key = squeeze_dots(&format!("'{}.{key}'", parts.join(".")));

        let removal = remove_range(ctx.text(Span::new(0, pair_span.end)), pair_span);
        ctx.report_with_fix(
            &Self::META,
            pair_span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::delete(removal),
                    Edit::replace(key_node.span(), new_key.into_bytes()),
                ],
            },
        );
    }
}

/// `node.value.to_s` for the basic literals that have one: strings and
/// symbols, plus (when `scope`) decimal integers and floats.
fn basic_value(ctx: &Context<'_>, node: &Node<'_>, scope: bool) -> Option<String> {
    if let Some(symbol) = node.as_symbol_node() {
        return Some(String::from_utf8_lossy(symbol.unescaped()).into_owned());
    }
    if let Some(string) = node.as_string_node() {
        return Some(String::from_utf8_lossy(string.unescaped()).into_owned());
    }
    if !scope {
        return None;
    }
    if let Some(integer) = node.as_integer_node() {
        if !integer.is_decimal() {
            return None;
        }
        let text = String::from_utf8_lossy(ctx.text(node.span())).replace('_', "");
        return Some(text.trim_start_matches('+').to_string());
    }
    node.as_float_node().map(|float| format!("{:?}", float.value()))
}

/// `String#squeeze('.')`.
fn squeeze_dots(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous_dot = false;
    for ch in text.chars() {
        if ch == '.' && previous_dot {
            continue;
        }
        previous_dot = ch == '.';
        out.push(ch);
    }
    out
}

/// `range_with_surrounding_comma(range_with_surrounding_space(range, side:
/// :left), :left)`: `source` is the file up to the range's end.
fn remove_range(source: &[u8], range: Span) -> Span {
    let mut start = range.start as usize;
    while start > 0 && matches!(source[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    while start > 0 && source[start - 1] == b'\n' {
        start -= 1;
    }
    if start > 0 && source[start - 1] == b',' {
        start -= 1;
    }
    Span::new(u32::try_from(start).expect("offset exceeds u32"), range.end)
}

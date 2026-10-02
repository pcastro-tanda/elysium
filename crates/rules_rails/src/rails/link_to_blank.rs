//! `Rails/LinkToBlank`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/link_to_blank.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{AssocNode, CallNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Specify a `:rel` option containing noopener.";

/// Checks that `link_to` with a `target: "_blank"` have a `rel: "noopener"` option passed to them.
#[derive(Debug, Clone)]
pub struct LinkToBlank;

impl Rule for LinkToBlank {
    const META: RuleMeta = RuleMeta {
        name: "Rails/LinkToBlank",
        department: Department::Rails,
        summary: "Checks that `link_to` with a `target: \"_blank\"` have a `rel: \"noopener\"` option passed to them.",
        explanation: "Checks that `link_to` (and `link_to_if` / `link_to_unless`) with a \
                      `target: \"_blank\"` have a `rel: \"noopener\"` option passed to them.\n\n\
                      ```ruby\n# bad\nlink_to \"Click here\", \"https://www.example.com\", target: \"_blank\"\n\n\
                      # good\nlink_to \"Click here\", \"https://www.example.com\", target: \"_blank\", rel: \"noopener\"\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation()
            || !matches!(call.name().as_slice(), b"link_to" | b"link_to_if" | b"link_to_unless")
        {
            return;
        }
        // `node.each_child_node(:hash)`: the receiver and the arguments.
        let mut hashes: Vec<Vec<AssocNode<'_>>> = Vec::new();
        let mut children: Vec<Node<'_>> = call.receiver().into_iter().collect();
        if let Some(arguments) = call.arguments() {
            children.extend(arguments.arguments().iter());
        }
        for child in &children {
            let elements = if let Some(hash) = child.as_hash_node() {
                hash.elements()
            } else if let Some(hash) = child.as_keyword_hash_node() {
                hash.elements()
            } else {
                continue;
            };
            hashes.push(elements.iter().filter_map(|e| e.as_assoc_node()).collect());
        }
        for options in &hashes {
            let Some(blank) = options.iter().find(|pair| is_blank_target(pair)) else { continue };
            if options.iter().any(includes_noopener) {
                continue;
            }
            let span = blank.as_node().span();
            let edit = autocorrect(&call, ctx, blank, &hashes);
            ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix { applicability: Applicability::Safe, edits: vec![edit] },
            );
        }
    }
}

/// The unescaped text of a `str` or `sym` node, and whether it is a symbol.
fn literal(node: &Node<'_>) -> Option<(Vec<u8>, bool)> {
    if let Some(sym) = node.as_symbol_node() {
        return Some((sym.unescaped().to_vec(), true));
    }
    node.as_string_node().map(|s| (s.unescaped().to_vec(), false))
}

fn key_is(pair: &AssocNode<'_>, name: &[u8]) -> bool {
    literal(&pair.key()).is_some_and(|(text, _)| text == name)
}

/// `(pair {(sym :target) (str "target")} {(str "_blank") (sym :_blank)})`.
fn is_blank_target(pair: &AssocNode<'_>) -> bool {
    key_is(pair, b"target") && literal(&pair.value()).is_some_and(|(text, _)| text == b"_blank")
}

/// `(pair {(sym :rel) (str "rel")} ({str sym} #contains_noopener?))`.
fn includes_noopener(pair: &AssocNode<'_>) -> bool {
    if !key_is(pair, b"rel") {
        return false;
    }
    literal(&pair.value()).is_some_and(|(text, _)| {
        String::from_utf8_lossy(&text)
            .split_whitespace()
            .any(|word| word == "noopener" || word == "noreferrer")
    })
}

/// `(pair {(sym :rel) (str "rel")} (str _))`.
fn is_rel_string(pair: &AssocNode<'_>) -> bool {
    key_is(pair, b"rel") && pair.value().as_string_node().is_some()
}

fn autocorrect(
    call: &CallNode<'_>,
    ctx: &Context<'_>,
    blank: &AssocNode<'_>,
    hashes: &[Vec<AssocNode<'_>>],
) -> Edit {
    let rel = hashes.iter().find_map(|options| options.iter().find(|p| is_rel_string(p)));
    if let Some(rel) = rel {
        // `append_to_rel`: replace the string's inside with `"#{value} noopener"`.
        let value = rel.value();
        let span = value.span();
        let existing = value.as_string_node().map(|s| s.unescaped().to_vec()).unwrap_or_default();
        let mut text = existing;
        text.extend_from_slice(b" noopener");
        return Edit::replace(Span::new(span.start + 1, span.end - 1), text);
    }
    // `add_rel`.
    let value_source = ctx.text(blank.value().span());
    let opening = value_source[0];
    let mut text = format!(", rel: {}noopener", opening as char).into_bytes();
    if opening != b':' {
        text.push(opening);
    }
    let end = last_argument_end(call);
    Edit::replace(Span::new(end, end), text)
}

/// End of the source RuboCop inserts after: the last pair of a trailing
/// hash argument, else the last argument itself.
fn last_argument_end(call: &CallNode<'_>) -> u32 {
    if let Some(block) = call.block() {
        if block.as_block_argument_node().is_some() {
            return block.span().end;
        }
    }
    let last = call.arguments().and_then(|a| a.arguments().iter().last());
    let Some(last) = last else { return call.as_node().span().end };
    let elements = if let Some(hash) = last.as_hash_node() {
        Some(hash.elements())
    } else {
        last.as_keyword_hash_node().map(|hash| hash.elements())
    };
    if let Some(elements) = elements {
        if let Some(pair) = elements.iter().filter_map(|e| e.as_assoc_node()).last() {
            return pair.as_node().span().end;
        }
    }
    last.span().end
}

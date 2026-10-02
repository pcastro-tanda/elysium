//! `Rails/EnumUniqueness`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/enum_uniqueness.rb` (with core's `Duplication`
//! mixin).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Looks for duplicate values in enum declarations.
#[derive(Debug, Clone)]
pub struct EnumUniqueness;

impl Rule for EnumUniqueness {
    const META: RuleMeta = RuleMeta {
        name: "Rails/EnumUniqueness",
        department: Department::Rails,
        summary: "Avoid duplicate integers in hash-syntax `enum` declaration.",
        explanation: "Looks for duplicate values in enum declarations.\n\n```ruby\n# bad\n\
                      enum :status, { active: 0, archived: 0 }\n\n# good\n\
                      enum :status, { active: 0, archived: 1 }\n\n# bad\n\
                      enum :status, [:active, :archived, :active]\n\n# good\n\
                      enum :status, [:active, :archived]\n\n# bad\n\
                      enum status: { active: 0, archived: 0 }\n\n# good\n\
                      enum status: { active: 0, archived: 1 }\n\n# bad\n\
                      enum status: [:active, :archived, :active]\n\n# good\n\
                      enum status: [:active, :archived]\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Duplicates are found by comparing literal values (strings and symbols by \
                      content, everything else by source text), where RuboCop compares ASTs; \
                      `1` and `0x1` are therefore different.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"enum" || call.receiver().is_some() {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let arguments: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let block_pass = call.block().is_some_and(|block| block.as_block_argument_node().is_some());

        // `(send nil? :enum $_ ${array hash} ...)`.
        if let [key, values, ..] = arguments.as_slice() {
            if is_array_or_hash(values) {
                check(key, values, ctx);
            }
        }
        // `(send nil? :enum (hash $...))`.
        if let ([hash], false) = (arguments.as_slice(), block_pass) {
            for (key, values) in hash_pairs(hash) {
                if is_array_or_hash(&values) {
                    check(&key, &values, ctx);
                }
            }
        }
    }
}

fn is_array_or_hash(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ArrayNode | NodeKind::HashNode | NodeKind::KeywordHashNode)
}

/// The `key`/`value` of each `pair` in a hash literal (empty for other
/// nodes).
fn hash_pairs<'pr>(hash: &Node<'pr>) -> Vec<(Node<'pr>, Node<'pr>)> {
    let elements = if let Some(hash) = hash.as_hash_node() {
        hash.elements()
    } else if let Some(hash) = hash.as_keyword_hash_node() {
        hash.elements()
    } else {
        return Vec::new();
    };
    elements
        .iter()
        .filter_map(|element| {
            let assoc = element.as_assoc_node()?;
            Some((assoc.key(), assoc.value()))
        })
        .collect()
}

/// `args.values`: an array's elements or a hash's pair values.
fn values<'pr>(node: &Node<'pr>) -> Vec<Node<'pr>> {
    match node.as_array_node() {
        Some(array) => array.elements().iter().collect(),
        None => hash_pairs(node).into_iter().map(|(_, value)| value).collect(),
    }
}

/// Node equality for grouping: kind plus literal content.
fn identity(node: &Node<'_>, ctx: &Context<'_>) -> (NodeKind, Vec<u8>) {
    let content = if let Some(string) = node.as_string_node() {
        string.unescaped().to_vec()
    } else if let Some(symbol) = node.as_symbol_node() {
        symbol.unescaped().to_vec()
    } else {
        ctx.text(node.span()).to_vec()
    };
    (node.kind(), content)
}

/// `message`'s `enum_name`.
fn enum_name(key: &Node<'_>, ctx: &Context<'_>) -> String {
    let name = if let Some(string) = key.as_string_node() {
        string.unescaped().to_vec()
    } else if let Some(symbol) = key.as_symbol_node() {
        symbol.unescaped().to_vec()
    } else {
        ctx.text(key.span()).to_vec()
    };
    String::from_utf8_lossy(&name).into_owned()
}

/// `consecutive_duplicates(args.values)`, each reported.
fn check(key: &Node<'_>, args: &Node<'_>, ctx: &mut Context<'_>) {
    let items = values(args);
    let identities: Vec<_> = items.iter().map(|item| identity(item, ctx)).collect();
    let mut offenses: Vec<Span> = Vec::new();
    let mut seen: Vec<&(NodeKind, Vec<u8>)> = Vec::new();
    for (item, id) in items.iter().zip(&identities) {
        if seen.contains(&id) {
            offenses.push(item.span());
        } else {
            seen.push(id);
        }
    }
    if offenses.is_empty() {
        return;
    }
    let name = enum_name(key, ctx);
    for span in offenses {
        let value = String::from_utf8_lossy(ctx.text(span)).into_owned();
        ctx.report(
            &EnumUniqueness::META,
            span,
            format!("Duplicate value `{value}` found in `{name}` enum declaration."),
        );
    }
}

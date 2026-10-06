//! `Rails/UniqueValidationWithoutIndex`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/unique_validation_without_index.rb` (with its
//! `ActiveRecordHelper` mixin).

use std::collections::BTreeSet;
use std::sync::Arc;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::{AssocNode, CallNode};
use ruby_ast::{Node, NodeKind};

use crate::active_record_helper::{class_info, enclosing_class, ClassInfo};
use crate::schema::Schema;

const MSG: &str = "Uniqueness validation should have a unique index on the database column.";

/// Checks that a uniqueness validation has a unique index on the database column.
#[derive(Debug, Clone)]
pub struct UniqueValidationWithoutIndex {
    /// The parsed `db/schema.rb`; the cop does nothing without one.
    schema: Option<Arc<Schema>>,
    /// Model classes already described this file.
    classes: Vec<ClassInfo>,
}

impl Rule for UniqueValidationWithoutIndex {
    const META: RuleMeta = RuleMeta {
        name: "Rails/UniqueValidationWithoutIndex",
        department: Department::Rails,
        summary: "Uniqueness validation should have a unique index on the database column.",
        explanation: "Checks for uniqueness validations of columns that have no unique index \
                      in `db/schema.rb`.\n\nValidating uniqueness in the application alone is \
                      racy; the database should enforce it with a unique index. The cop reads \
                      the schema the working directory's `db/schema.rb` dumps and does nothing \
                      without one.\n\n\
                      ```ruby\n# bad - rails_helper.rb is the only guard\nclass Account < ApplicationRecord\n  validates :email, uniqueness: true\nend\n\n\
                      # good - with `t.index [\"email\"], unique: true` in db/schema.rb\nclass Account < ApplicationRecord\n  validates :email, uniqueness: true\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "The schema is read from the nearest `db/schema.rb` above the working \
                      directory when the run starts; `db/structure.sql` is not read.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let schema = options.db_schema().and_then(|source| Schema::parse(source)).map(Arc::new);
        Ok(Self { schema, classes: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(schema) = self.schema.clone() else { return };
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"validates" || call.is_safe_navigation() {
            return;
        }
        let Some(uniqueness) = uniqueness_part(&call) else { return };
        if matches!(uniqueness.kind_tag(), Tag::Falsey) || condition_part(&call, &uniqueness) {
            return;
        }
        let Some(class_span) = enclosing_class(ctx).map(|info| info.span) else { return };
        let index = if let Some(index) = self.classes.iter().position(|c| c.span == class_span) {
            index
        } else {
            let Some(info) = class_info(ctx, class_span) else { return };
            self.classes.push(info);
            self.classes.len() - 1
        };
        let class = &self.classes[index];
        let table = schema.table_by(&class.table_name);
        let Some(names) = column_names(&call, &uniqueness, class, table) else { return };
        if with_index(&schema, class, &names) {
            return;
        }
        ctx.report(&Self::META, call_span_excluding_block(&call), MSG);
    }
}

/// What the `uniqueness:` value is, as far as the cop cares.
enum Tag {
    Falsey,
    Hash,
    Other,
}

/// The value of the `uniqueness:` pair.
struct Uniqueness<'a>(Node<'a>);

impl Uniqueness<'_> {
    fn kind_tag(&self) -> Tag {
        if self.0.as_false_node().is_some() || self.0.as_nil_node().is_some() {
            Tag::Falsey
        } else if self.hash_pairs().is_some() {
            Tag::Hash
        } else {
            Tag::Other
        }
    }

    fn hash_pairs(&self) -> Option<Vec<AssocNode<'_>>> {
        pairs_of(&self.0)
    }
}

/// `each_pair` of a hash node.
fn pairs_of<'a>(node: &Node<'a>) -> Option<Vec<AssocNode<'a>>> {
    let elements = if let Some(hash) = node.as_keyword_hash_node() {
        hash.elements()
    } else {
        node.as_hash_node()?.elements()
    };
    Some(elements.iter().filter_map(|e| e.as_assoc_node()).collect())
}

fn symbol_key(pair: &AssocNode<'_>) -> Option<Vec<u8>> {
    pair.key().as_symbol_node().map(|k| k.unescaped().to_vec())
}

/// The last argument, when it is a hash (a `&block` argument counts as the
/// last argument in RuboCop's AST).
fn last_hash<'a>(call: &CallNode<'a>) -> Option<Vec<AssocNode<'a>>> {
    if call.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
        return None;
    }
    let last = call.arguments()?.arguments().iter().last()?;
    pairs_of(&last)
}

/// `uniqueness_part`.
fn uniqueness_part<'a>(call: &CallNode<'a>) -> Option<Uniqueness<'a>> {
    last_hash(call)?
        .into_iter()
        .find(|pair| symbol_key(pair).is_some_and(|key| key == b"uniqueness"))
        .map(|pair| Uniqueness(pair.value()))
}

/// `condition_part?`.
fn condition_part(call: &CallNode<'_>, uniqueness: &Uniqueness<'_>) -> bool {
    let Some(pairs) = last_hash(call) else { return false };
    if has_key(&pairs, &[b"if", b"unless"]) {
        return true;
    }
    uniqueness.hash_pairs().is_some_and(|pairs| has_key(&pairs, &[b"if", b"unless", b"conditions"]))
}

fn has_key(pairs: &[AssocNode<'_>], keys: &[&[u8]]) -> bool {
    pairs.iter().any(|pair| symbol_key(pair).is_some_and(|key| keys.contains(&key.as_slice())))
}

fn text(node: &Node<'_>) -> Option<String> {
    if let Some(symbol) = node.as_symbol_node() {
        return Some(String::from_utf8_lossy(symbol.unescaped()).into_owned());
    }
    node.as_string_node().map(|string| String::from_utf8_lossy(string.unescaped()).into_owned())
}

/// `column_names`: the validated column plus its scope, each resolved
/// against the table; `None` when any cannot be.
fn column_names(
    call: &CallNode<'_>,
    uniqueness: &Uniqueness<'_>,
    class: &ClassInfo,
    table: Option<&crate::schema::Table>,
) -> Option<BTreeSet<String>> {
    let first = call.arguments()?.arguments().iter().next()?;
    let mut names = vec![text(&first)?];
    if let Some(scope) = column_names_from_scope(uniqueness) {
        names.extend(scope);
    }
    let mut resolved = BTreeSet::new();
    for name in &names {
        resolved.extend(class.resolve_relation_into_column(name, table)?);
    }
    Some(resolved)
}

/// `column_names_from_scope`.
fn column_names_from_scope(uniqueness: &Uniqueness<'_>) -> Option<Vec<String>> {
    let pairs = uniqueness.hash_pairs()?;
    let scope =
        pairs.iter().find(|pair| symbol_key(pair).is_some_and(|key| key == b"scope"))?.value();
    // `unfreeze_scope`: `scope.freeze` is its receiver.
    let scope = match scope.as_call_node() {
        Some(freeze) if !freeze.is_safe_navigation() && freeze.name().as_slice() == b"freeze" => {
            freeze.receiver()?
        }
        _ => scope,
    };
    if let Some(name) = text(&scope) {
        return Some(vec![name]);
    }
    let array = scope.as_array_node()?;
    array.elements().iter().map(|element| text(&element)).collect()
}

/// `with_index?`.
fn with_index(schema: &Schema, class: &ClassInfo, names: &BTreeSet<String>) -> bool {
    let table_indices =
        schema.table_by(&class.table_name).map_or(&[][..], |t| t.indices.as_slice());
    let add_indices = schema.add_indices_by(&class.table_name).map(|add| &add.index);
    table_indices.iter().chain(add_indices).any(|index| {
        index.unique
            && (index.columns.iter().cloned().collect::<BTreeSet<_>>() == *names
                || index.expression.as_ref().is_some_and(|expression| {
                    names.iter().all(|name| expression.contains(name.as_str()))
                }))
    })
}

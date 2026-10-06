//! `Rails/DuplicateAssociation`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/duplicate_association.rb` (with the
//! `ClassSendNodeHelper` and `ActiveRecordHelper` mixins).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Looks for associations that have been defined multiple times in the same
/// file.
#[derive(Debug, Clone)]
pub struct DuplicateAssociation;

impl Rule for DuplicateAssociation {
    const META: RuleMeta = RuleMeta {
        name: "Rails/DuplicateAssociation",
        department: Department::Rails,
        summary: "Don't repeat associations in a model.",
        explanation: "Looks for associations that have been defined multiple times in the same \
                      file.\n\nWhen an association is defined multiple times on a model, Active \
                      Record overrides the previously defined association with the new one. \
                      Because of this, this cop's autocorrection simply keeps the last of any \
                      duplicates and discards the rest.\n\n```ruby\n# bad\nbelongs_to \
                      :foo\nbelongs_to :bar\nhas_one :foo\n\n# good\nbelongs_to :bar\nhas_one \
                      :foo\n\n# bad\nhas_many :foo, class_name: 'Foo'\nhas_many :bar, \
                      class_name: 'Foo'\nhas_one :baz\n\n# good\nhas_many :bar, class_name: \
                      'Foo'\nhas_one :foo\n```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(class) = node.as_class_node() else { return };
        if !class.superclass().is_some_and(|parent| is_active_record(&parent)) {
            return;
        }
        let Some(body) = class.body() else { return };
        let statements: Vec<Node<'_>> = match body.as_statements_node() {
            Some(statements) => statements.body().iter().collect(),
            None => vec![body],
        };

        let mut associations = Vec::new();
        for statement in &statements {
            if let Some(association) = Association::of(statement, ctx) {
                associations.push(association);
            }
        }

        // Name groups (keyed by the symbolised first argument).
        let mut by_name: Vec<(Vec<u8>, Vec<&Association>)> = Vec::new();
        for association in &associations {
            match by_name.iter_mut().find(|(name, _)| *name == association.name) {
                Some((_, group)) => group.push(association),
                None => by_name.push((association.name.clone(), vec![association])),
            }
        }
        for (name, group) in &by_name {
            if group.len() > 1 {
                register(ctx, &String::from_utf8_lossy(name), group, false);
            }
        }

        // `class_name:` groups, `belongs_to` excluded.
        let mut by_class: Vec<(Vec<u8>, Vec<&Association>)> = Vec::new();
        for association in associations.iter().filter(|a| !a.belongs_to) {
            let Some(class_name) = &association.class_name else { continue };
            match by_class.iter_mut().find(|(name, _)| name == class_name) {
                Some((_, group)) => group.push(association),
                None => by_class.push((class_name.clone(), vec![association])),
            }
        }
        for (class_name, group) in &by_class {
            if group.len() > 1 {
                register(ctx, &String::from_utf8_lossy(class_name), group, true);
            }
        }
    }
}

struct Association {
    span: Span,
    name: Vec<u8>,
    belongs_to: bool,
    /// Source of the `class_name:` value when it is the only other argument
    /// and a lone `class_name:` pair.
    class_name: Option<Vec<u8>>,
}

impl Association {
    /// `(send nil? {:belongs_to :has_one :has_many :has_and_belongs_to_many}
    /// ({sym str} $_) $...)`.
    fn of(node: &Node<'_>, ctx: &Context<'_>) -> Option<Self> {
        let call = node.as_call_node()?;
        if call.receiver().is_some()
            || call.block().is_some_and(|block| block.as_block_node().is_some())
        {
            return None;
        }
        let name = call.name();
        if !matches!(
            name.as_slice(),
            b"belongs_to" | b"has_one" | b"has_many" | b"has_and_belongs_to_many"
        ) {
            return None;
        }
        let arguments: Vec<Node<'_>> = call.arguments()?.arguments().iter().collect();
        let (first, rest) = arguments.split_first()?;
        let association = if let Some(symbol) = first.as_symbol_node() {
            symbol.unescaped().to_vec()
        } else {
            first.as_string_node()?.unescaped().to_vec()
        };
        let class_name = match rest {
            [options] => class_name_of(options, ctx),
            _ => None,
        };
        Some(Self {
            span: node.span(),
            name: association,
            belongs_to: name.as_slice() == b"belongs_to",
            class_name,
        })
    }
}

/// `(hash (pair (sym :class_name) $_))`, as the value's source.
fn class_name_of(node: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
    let elements: Vec<Node<'_>> = if let Some(hash) = node.as_keyword_hash_node() {
        hash.elements().iter().collect()
    } else {
        node.as_hash_node()?.elements().iter().collect()
    };
    let [element] = elements.as_slice() else { return None };
    let pair = element.as_assoc_node()?;
    let key = pair.key();
    if key.as_symbol_node()?.unescaped() != b"class_name" {
        return None;
    }
    Some(ctx.text(pair.value().span()).to_vec())
}

fn register(ctx: &mut Context<'_>, name: &str, group: &[&Association], class_name: bool) {
    let message = if class_name {
        format!(
            "Association `class_name: {name}` is defined multiple times. Don't repeat \
             associations."
        )
    } else {
        format!("Association `{name}` is defined multiple times. Don't repeat associations.")
    };
    let Some(last) = group.last() else { return };
    let last_source = ctx.text(last.span).to_vec();
    for (index, association) in group.iter().enumerate() {
        let edit = if index == 0 {
            Edit::replace(association.span, last_source.clone())
        } else {
            Edit::replace(ctx.whole_lines(association.span), Vec::new())
        };
        ctx.report_with_fix(
            &DuplicateAssociation::META,
            association.span,
            message.clone(),
            Fix { applicability: Applicability::Safe, edits: vec![edit] },
        );
    }
}

/// `active_record?`: `ApplicationRecord` or `ActiveRecord::Base`, each
/// optionally rooted at `::`.
fn is_active_record(node: &Node<'_>) -> bool {
    if is_bare_or_toplevel_const(node) {
        return const_name(node).as_deref() == Some("ApplicationRecord");
    }
    let Some(path) = node.as_constant_path_node() else { return false };
    path.name().is_some_and(|name| name.as_slice() == b"Base")
        && path.parent().is_some_and(|parent| {
            is_bare_or_toplevel_const(&parent)
                && const_name(&parent).as_deref() == Some("ActiveRecord")
        })
}

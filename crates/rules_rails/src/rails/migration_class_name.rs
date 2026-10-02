//! `Rails/MigrationClassName`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/migration_class_name.rb` (with its
//! `MigrationsHelper` mixin).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_ast::node::ClassNode;
use ruby_source::Span;

/// Makes sure that each migration file defines a migration class whose name
/// matches the file name.
#[derive(Debug, Clone)]
pub struct MigrationClassName;

impl Rule for MigrationClassName {
    const META: RuleMeta = RuleMeta {
        name: "Rails/MigrationClassName",
        department: Department::Rails,
        summary: "The class name of the migration should match its file name.",
        explanation: "Makes sure that each migration file defines a migration class whose name \
                      matches the file name. (e.g. `20220224111111_create_users.rb` should \
                      define `CreateUsers` class.)\n\n```ruby\n# db/migrate/\
                      20220224111111_create_users.rb\n\n# bad\nclass SellBooks < \
                      ActiveRecord::Migration[7.0]\nend\n\n# good\nclass CreateUsers < \
                      ActiveRecord::Migration[7.0]\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(class) = node.as_class_node() else { return };
        if !is_migration_class(&class) {
            return;
        }
        let basename =
            basename_without_timestamp_and_suffix(&ctx.source().path().to_string_lossy());

        let path = class.constant_path();
        let identifier: Span = path
            .as_constant_path_node()
            .map_or_else(|| path.span(), |path| path.name_loc().span());
        let camelized = camelize(&basename);
        if ctx.text(identifier).eq_ignore_ascii_case(camelized.as_bytes()) {
            return;
        }

        let message = format!("Replace with `{camelized}` that matches the file name.");
        ctx.report_with_fix(
            &Self::META,
            identifier,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(identifier, camelized.into_bytes())],
            },
        );
    }
}

/// `MigrationsHelper#migration_class?`:
/// `(class (const {nil? cbase} _) (send (const (const {nil? cbase}
/// :ActiveRecord) :Migration) :[] (float _)) _)`.
fn is_migration_class(class: &ClassNode<'_>) -> bool {
    let name = class.constant_path();
    if name.as_constant_read_node().is_none()
        && !(name.as_constant_path_node().is_some() && is_bare_or_toplevel_const(&name))
    {
        return false;
    }
    let Some(superclass) = class.superclass() else { return false };
    let Some(call) = superclass.as_call_node() else { return false };
    if call.name().as_slice() != b"[]" || call.is_safe_navigation() || call.block().is_some() {
        return false;
    }
    let Some(arguments) = call.arguments() else { return false };
    let mut arguments = arguments.arguments().iter();
    let (Some(argument), None) = (arguments.next(), arguments.next()) else { return false };
    if argument.as_float_node().is_none() {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    let Some(path) = receiver.as_constant_path_node() else { return false };
    path.name().is_some_and(|name| name.as_slice() == b"Migration")
        && path.parent().is_some_and(|parent| {
            is_bare_or_toplevel_const(&parent)
                && const_name(&parent).as_deref() == Some("ActiveRecord")
        })
}

fn basename_without_timestamp_and_suffix(filepath: &str) -> String {
    let file = std::path::Path::new(filepath)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    // `File.basename(filepath, '.rb')` keeps a basename that is only `.rb`.
    let basename = match file.strip_suffix(".rb") {
        Some(stripped) if !stripped.is_empty() => stripped,
        _ => file.as_str(),
    };
    // `remove_gem_suffix`: `sub(/\..+\z/, '')`.
    let basename = match basename.find('.') {
        Some(index) if index + 1 < basename.len() => &basename[..index],
        _ => basename,
    };
    // `sub(/\A\d+_/, '')`.
    let digits = basename.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && basename.as_bytes().get(digits) == Some(&b'_') {
        basename[digits + 1..].to_owned()
    } else {
        basename.to_owned()
    }
}

/// `word.split('_').map(&:capitalize).join`.
fn camelize(word: &str) -> String {
    word.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect()
            })
        })
        .collect()
}

//! `db/schema.rb`, ported from rubocop-rails'
//! `lib/rubocop/rails/schema_loader/schema.rb`.
//!
//! The CLI finds the file the way `SchemaLoader.db_schema_path` does (walking
//! up from the working directory) and hands its source to every rule as
//! [`linter::RuleOptions::db_schema`]; a rule that reads the schema calls
//! [`Schema::parse`] on it from `configure`. `None` -- no file, or one that
//! does not parse to a single `ActiveRecord::Schema.define ... do ... end`
//! block -- makes the cop skip, as upstream's `schema` is `nil` then.

use ruby_ast::node::{CallNode, HashNode};
use ruby_ast::{Node, Parsed};
use ruby_source::SourceFile;

/// `RuboCop::Rails::SchemaLoader::Schema`.
#[derive(Debug, Clone, Default)]
pub struct Schema {
    tables: Vec<Table>,
    add_indices: Vec<AddIndex>,
}

/// `Schema::Table`: a `create_table` block.
#[derive(Debug, Clone)]
pub struct Table {
    /// The table name (`create_table`'s first argument).
    pub name: String,
    /// Every non-`index` call in the block.
    pub columns: Vec<Column>,
    /// Every `index` call in the block.
    pub indices: Vec<Index>,
}

/// `Schema::Column`: any non-`index` call in a `create_table` block.
#[derive(Debug, Clone)]
pub struct Column {
    /// The first argument's string/symbol value, `None` for another node.
    pub name: Option<String>,
    /// The method name (`string`, `bigint`, ...).
    pub column_type: String,
    /// `Some(!value)` for a `null:` option, `None` without one.
    pub not_null: Option<bool>,
}

/// `Schema::Index`.
#[derive(Debug, Clone, Default)]
pub struct Index {
    /// The `name:` option.
    pub name: Option<String>,
    /// The `["a", "b"]` form's names; empty for an expression index.
    pub columns: Vec<String>,
    /// The string form's expression (`"lower(address)"`).
    pub expression: Option<String>,
    /// Upstream sets this to `true` whenever a `unique:` option is present,
    /// whatever its value.
    pub unique: bool,
}

/// `Schema::AddIndex`: a top-level `add_index` (Rails 4.2 dumps).
#[derive(Debug, Clone)]
pub struct AddIndex {
    /// The table the index is added to.
    pub table_name: String,
    /// The index itself.
    pub index: Index,
}

impl Schema {
    /// Parses the source of a `db/schema.rb`. `None` when it is empty, has a
    /// syntax error, or is not one block (RuboCop's `ProcessedSource#ast` is
    /// `nil` for the first two, and `Schema.new` raises for the last).
    #[must_use]
    pub fn parse(source: &str) -> Option<Self> {
        let file = SourceFile::new("db/schema.rb", source.as_bytes().to_vec());
        let parsed = Parsed::parse(&file);
        if parsed.has_errors() {
            return None;
        }
        let root = parsed.root();
        let statements = root.as_program_node()?.statements();
        let mut body = statements.body().iter();
        let (Some(only), None) = (body.next(), body.next()) else { return None };
        let block = only.as_call_node()?.block()?.as_block_node()?;
        let mut schema = Self::default();
        let Some(body) = block.body() else { return Some(schema) };
        let statements: Vec<Node<'_>> = match body.as_statements_node() {
            Some(statements) => statements.body().iter().collect(),
            None => vec![body],
        };
        let multiple = statements.len() > 1;
        for statement in &statements {
            if let Some(table) = create_table(statement) {
                schema.tables.push(table);
            }
        }
        // `each_add_index` walks `ast.body.children`, which are the
        // statements only when the body is a `begin`.
        if multiple {
            for statement in &statements {
                if let Some(add_index) = add_index(statement) {
                    schema.add_indices.push(add_index);
                }
            }
        }
        Some(schema)
    }

    /// `Schema#table_by(name:)`.
    #[must_use]
    pub fn table_by(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|table| table.name == name)
    }

    /// `Schema#add_indices_by(table_name:)`.
    pub fn add_indices_by<'a>(&'a self, table_name: &'a str) -> impl Iterator<Item = &'a AddIndex> {
        self.add_indices.iter().filter(move |add_index| add_index.table_name == table_name)
    }
}

impl Table {
    /// `Table#with_column?(name:)`.
    #[must_use]
    pub fn with_column(&self, name: &str) -> bool {
        self.columns.iter().any(|column| column.name.as_deref() == Some(name))
    }
}

fn text(node: &Node<'_>) -> Option<String> {
    if let Some(symbol) = node.as_symbol_node() {
        return Some(String::from_utf8_lossy(symbol.unescaped()).into_owned());
    }
    node.as_string_node().map(|string| String::from_utf8_lossy(string.unescaped()).into_owned())
}

fn arguments<'a>(call: &CallNode<'a>) -> Vec<Node<'a>> {
    call.arguments().map(|arguments| arguments.arguments().iter().collect()).unwrap_or_default()
}

/// A `send`: a call with no attached block (a `block-pass` is an argument there).
fn is_send(call: &CallNode<'_>) -> bool {
    call.block().is_none_or(|block| block.as_block_argument_node().is_some())
        && !call.is_safe_navigation()
}

/// The `key => value` pairs of a trailing hash argument.
fn options<'a>(last: Option<&Node<'a>>) -> Vec<(Node<'a>, Node<'a>)> {
    let Some(last) = last else { return Vec::new() };
    let elements = if let Some(hash) = last.as_keyword_hash_node() {
        hash.elements()
    } else if let Some(hash) = last.as_hash_node() {
        let hash: HashNode<'_> = hash;
        hash.elements()
    } else {
        return Vec::new();
    };
    elements
        .iter()
        .filter_map(|e| e.as_assoc_node())
        .map(|pair| (pair.key(), pair.value()))
        .collect()
}

fn create_table(node: &Node<'_>) -> Option<Table> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"create_table" {
        return None;
    }
    let block = call.block()?.as_block_node()?;
    let name = text(arguments(&call).first()?)?;
    let contents: Vec<Node<'_>> = match block.body() {
        None => Vec::new(),
        Some(body) => match body.as_statements_node() {
            Some(statements) => statements.body().iter().collect(),
            None => vec![body],
        },
    };
    let mut table = Table { name, columns: Vec::new(), indices: Vec::new() };
    for child in &contents {
        let Some(call) = child.as_call_node() else { continue };
        if !is_send(&call) {
            continue;
        }
        let args = arguments(&call);
        if call.name().as_slice() == b"index" {
            if let Some(first) = args.first() {
                table.indices.push(build_index(first, &args));
            }
        } else {
            let mut not_null = None;
            for (key, value) in options(args.last()) {
                if text(&key).as_deref() == Some("null") {
                    not_null = Some(value.as_true_node().is_none());
                }
            }
            table.columns.push(Column {
                name: args.first().and_then(text),
                column_type: String::from_utf8_lossy(call.name().as_slice()).into_owned(),
                not_null,
            });
        }
    }
    Some(table)
}

/// `Index#initialize` over the columns-or-expression node and the whole argument list.
fn build_index(columns: &Node<'_>, args: &[Node<'_>]) -> Index {
    let mut index = Index::default();
    if let Some(array) = columns.as_array_node() {
        index.columns = array.elements().iter().filter_map(|e| text(&e)).collect();
    } else {
        index.expression = text(columns);
    }
    for (key, value) in options(args.last()) {
        match text(&key).as_deref() {
            Some("name") => index.name = text(&value),
            Some("unique") => index.unique = true,
            _ => {}
        }
    }
    index
}

fn add_index(node: &Node<'_>) -> Option<AddIndex> {
    let call = node.as_call_node()?;
    if !is_send(&call) || call.name().as_slice() != b"add_index" {
        return None;
    }
    let args = arguments(&call);
    let table_name = text(args.first()?)?;
    let index = build_index(args.get(1)?, &args);
    Some(AddIndex { table_name, index })
}

#[cfg(test)]
mod tests {
    use super::Schema;

    const SOURCE: &str = r#"
ActiveRecord::Schema[7.0].define(version: 2020_02_02_075409) do
  create_table "users", force: :cascade do |t|
    t.string "account", null: false
    t.bigint "org_id"
    t.index ["account", "org_id"], name: "idx_a", unique: true
    t.index "lower(account)", name: "idx_l"
    t.check_constraint nil, "x", name: "c"
  end
  add_index "users", ["org_id"], name: "idx_o", unique: false
end
"#;

    #[test]
    fn reads_tables_columns_indices_and_add_index() {
        let schema = Schema::parse(SOURCE).expect("parses");
        let users = schema.table_by("users").expect("table");
        assert!(users.with_column("account") && users.with_column("org_id"));
        assert_eq!(users.columns[0].not_null, Some(true));
        assert_eq!(users.columns[1].not_null, None);
        assert_eq!(users.indices[0].columns, ["account", "org_id"]);
        assert!(users.indices[0].unique);
        assert_eq!(users.indices[1].expression.as_deref(), Some("lower(account)"));
        assert!(!users.indices[1].unique);
        // Upstream marks any `unique:` option as unique, even `unique: false`.
        let added: Vec<_> = schema.add_indices_by("users").collect();
        assert_eq!(added.len(), 1);
        assert!(added[0].index.unique);
    }

    #[test]
    fn unusable_sources_are_no_schema() {
        assert!(Schema::parse("").is_none());
        assert!(Schema::parse("ActiveRecord::Schema.define do\n").is_none());
        assert!(Schema::parse("a = 1\nb = 2\n").is_none());
    }
}

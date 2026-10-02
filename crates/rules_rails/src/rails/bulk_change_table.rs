//! `Rails/BulkChangeTable`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/bulk_change_table.rb` (with its
//! `DatabaseTypeResolvable` mixin).

use linter::{
    Context, Department, FixAvailability, OptionError, OptionValue, Rule, RuleMeta, RuleOptions,
    Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG_FOR_CHANGE_TABLE: &str = "You can combine alter queries using `bulk: true` options.";

const MIGRATION_METHODS: [&[u8]; 3] = [b"change", b"up", b"down"];

const COMBINABLE_TRANSFORMATIONS: &[&[u8]] = &[
    b"primary_key",
    b"column",
    b"string",
    b"text",
    b"integer",
    b"bigint",
    b"float",
    b"decimal",
    b"numeric",
    b"datetime",
    b"timestamp",
    b"time",
    b"date",
    b"binary",
    b"boolean",
    b"json",
    b"virtual",
    b"remove",
    b"change",
    b"timestamps",
    b"remove_timestamps",
];

const COMBINABLE_ALTER_METHODS: &[&[u8]] = &[
    b"add_column",
    b"remove_column",
    b"remove_columns",
    b"change_column",
    b"add_timestamps",
    b"remove_timestamps",
];

const MYSQL_COMBINABLE_TRANSFORMATIONS: &[&[u8]] = &[b"rename", b"index", b"remove_index"];
const MYSQL_COMBINABLE_ALTER_METHODS: &[&[u8]] = &[b"rename_column", b"add_index", b"remove_index"];
const POSTGRESQL_COMBINABLE_TRANSFORMATIONS: &[&[u8]] = &[b"change_default"];
const POSTGRESQL_COMBINABLE_TRANSFORMATIONS_SINCE_6_1: &[&[u8]] = &[b"change_null"];
const POSTGRESQL_COMBINABLE_ALTER_METHODS: &[&[u8]] = &[b"change_column_default"];
const POSTGRESQL_COMBINABLE_ALTER_METHODS_SINCE_6_1: &[&[u8]] = &[b"change_column_null"];

/// `TargetRailsVersion::DEFAULT_RAILS_VERSION`, used when the configuration
/// states none.
const DEFAULT_RAILS_VERSION: f64 = 5.0;

/// `DatabaseTypeResolvable::MYSQL` / `POSTGRESQL`; any other `Database`
/// string is unsupported and so is represented by `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Database {
    Mysql,
    Postgresql,
}

/// Checks whether alter queries are combinable.
#[derive(Debug, Clone)]
pub struct BulkChangeTable {
    database: Option<Database>,
    /// `target_rails_version >= 5.2`, which PostgreSQL needs for bulk alter.
    postgresql_bulk_alter: bool,
    /// `target_rails_version >= 6.1`.
    since_6_1: bool,
}

impl Rule for BulkChangeTable {
    const META: RuleMeta = RuleMeta {
        name: "Rails/BulkChangeTable",
        department: Department::Rails,
        summary: "Check whether alter queries are combinable.",
        explanation: "Checks whether alter queries are combinable. If combinable queries are \
                      detected, it suggests to you to use `change_table` with `bulk: true` \
                      instead. This option causes the migration to generate a single ALTER \
                      TABLE statement combining multiple column alterations.\n\nThe `bulk` \
                      option is only supported on the MySQL and the PostgreSQL (5.2 later) \
                      adapter; thus it will automatically detect an adapter from \
                      `development` environment in `config/database.yml` or the environment \
                      variable `DATABASE_URL` when the `Database` option is not set. If the \
                      adapter is not `mysql2`, `trilogy`, `postgresql`, or `postgis`, this \
                      Cop ignores offenses.\n\n```ruby\n# bad\ndef change\n  add_column \
                      :users, :name, :string, null: false\n  add_column :users, :nickname, \
                      :string\nend\n\n# good\ndef change\n  change_table :users, bulk: true \
                      do |t|\n    t.string :name, null: false\n    t.string :nickname\n  \
                      end\nend\n```\n\n```ruby\n# bad\ndef change\n  change_table :users do \
                      |t|\n    t.string :name, null: false\n    t.string :nickname\n  end\n\
                      end\n\n# good\ndef change\n  change_table :users, bulk: true do |t|\n    \
                      t.string :name, null: false\n    t.string :nickname\n  end\nend\n\n\
                      # good\n# When you don't want to combine alter queries.\ndef change\n  \
                      change_table :users, bulk: false do |t|\n    t.string :name, null: \
                      false\n    t.string :nickname\n  end\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::DefNode, NodeKind::CallNode],
        config: &[
            linter::ConfigOption {
                name: "Database",
                default: linter::ConfigDefault::Nil,
                allowed: &[],
                doc: "Database adapter (`mysql` or `postgresql`); detected from \
                      `config/database.yml` or `DATABASE_URL` when unset.",
            },
            linter::ConfigOption {
                name: "SupportedDatabases",
                default: linter::ConfigDefault::StrList(&["mysql", "postgresql"]),
                allowed: &[],
                doc: "Databases that support bulk alter.",
            },
        ],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first. \
                      `config/database.yml` and `DATABASE_URL` are read relative to the \
                      working directory, as RuboCop does.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let version = target_rails_version(options);
        Ok(Self {
            database: resolve_database(options),
            postgresql_bulk_alter: version >= 5.2,
            since_6_1: version >= 6.1,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.support_bulk_alter() {
            return;
        }
        match node.kind() {
            NodeKind::DefNode => self.on_def(node, ctx),
            NodeKind::CallNode => self.on_send(node, ctx),
            _ => {}
        }
    }
}

impl BulkChangeTable {
    fn support_bulk_alter(&self) -> bool {
        match self.database {
            Some(Database::Mysql) => true,
            Some(Database::Postgresql) => self.postgresql_bulk_alter,
            None => false,
        }
    }

    fn is_combinable_alter_method(&self, name: &[u8]) -> bool {
        let in_list = |list: &[&[u8]]| list.contains(&name);
        in_list(COMBINABLE_ALTER_METHODS)
            || match self.database {
                Some(Database::Mysql) => in_list(MYSQL_COMBINABLE_ALTER_METHODS),
                Some(Database::Postgresql) => {
                    in_list(POSTGRESQL_COMBINABLE_ALTER_METHODS)
                        || (self.since_6_1 && in_list(POSTGRESQL_COMBINABLE_ALTER_METHODS_SINCE_6_1))
                }
                None => false,
            }
    }

    fn is_combinable_transformation(&self, name: &[u8]) -> bool {
        let in_list = |list: &[&[u8]]| list.contains(&name);
        in_list(COMBINABLE_TRANSFORMATIONS)
            || match self.database {
                Some(Database::Mysql) => in_list(MYSQL_COMBINABLE_TRANSFORMATIONS),
                Some(Database::Postgresql) => {
                    in_list(POSTGRESQL_COMBINABLE_TRANSFORMATIONS)
                        || (self.since_6_1
                            && in_list(POSTGRESQL_COMBINABLE_TRANSFORMATIONS_SINCE_6_1))
                }
                None => false,
            }
    }

    fn on_def(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        if !MIGRATION_METHODS.contains(&def.name().as_slice()) {
            return;
        }
        let Some(body) = def.body() else { return };

        let mut recorder = AlterMethodsRecorder::default();
        for child in body_children(&body) {
            match child {
                Some(call) if self.is_combinable_alter_method(call.name().as_slice()) => {
                    recorder.process(&call);
                }
                _ => recorder.flush(),
            }
        }
        for call in recorder.finish() {
            // `return unless table_node.is_a? BasicLiteralNode`
            let Some(table) = first_argument(&call).and_then(|arg| literal_value(&arg, ctx))
            else {
                continue;
            };
            let message = format!("You can use `change_table :{table}, bulk: true` to combine alter queries.");
            ctx.report(&Self::META, call_span_excluding_block(&call), message);
        }
    }

    fn on_send(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // `node.command?(:change_table)`: a receiverless `send`.
        if call.name().as_slice() != b"change_table" || call.receiver().is_some() {
            return;
        }
        if include_bulk_options(&call) {
            return;
        }
        let Some(block) = call.block().and_then(|block| block.as_block_node()) else { return };
        let Some(body) = block.body() else { return };

        let sends = send_nodes_from_change_table_block(&body);
        let count: usize = sends
            .iter()
            .map(|send| {
                if send.name().as_slice() == b"remove" {
                    arguments_of(send).iter().filter(|arg| !is_hash(arg)).count()
                } else {
                    usize::from(self.is_combinable_transformation(send.name().as_slice()))
                }
            })
            .sum();
        if count > 1 {
            ctx.report(&Self::META, call_span_excluding_block(&call), MSG_FOR_CHANGE_TABLE);
        }
    }
}

/// `AlterMethodsRecorder`: records runs of combinable alter methods on the
/// same table.
#[derive(Default)]
struct AlterMethodsRecorder<'pr> {
    nodes: Vec<(CallNode<'pr>, String)>,
    offensive: Vec<CallNode<'pr>>,
}

impl<'pr> AlterMethodsRecorder<'pr> {
    fn process(&mut self, call: &CallNode<'pr>) {
        let table = first_argument(call).and_then(|arg| table_text(&arg));
        match table {
            Some(table) => {
                if !self.nodes.iter().all(|(_, existing)| *existing == table) {
                    self.flush();
                }
                self.nodes.push((call.clone(), table));
            }
            None => self.flush(),
        }
    }

    fn flush(&mut self) {
        if self.nodes.len() > 1 {
            self.offensive.push(self.nodes[0].0.clone());
        }
        self.nodes.clear();
    }

    fn finish(mut self) -> Vec<CallNode<'pr>> {
        self.flush();
        self.offensive
    }
}

/// `send_type?`: not `&.`, and not the `send` of a literal-block `block`.
fn as_send<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    let literal_block = call.block().is_some_and(|block| block.as_block_node().is_some());
    (!call.is_safe_navigation() && !literal_block).then_some(call)
}

/// The whitequark children of `call` that `node.child_nodes` would yield:
/// receiver, arguments and block-pass. `None` marks a child that is not a
/// `send`.
fn call_children<'pr>(call: &CallNode<'pr>) -> Vec<Option<CallNode<'pr>>> {
    let literal_block = call.block().is_some_and(|block| block.as_block_node().is_some());
    if literal_block {
        // `(block (send ...) args body)`: the `send` is its first child.
        return vec![Some(call.clone()), None, None];
    }
    let mut out = Vec::new();
    if let Some(receiver) = call.receiver() {
        out.push(as_send(&receiver));
    }
    out.extend(arguments_of(call).iter().map(as_send));
    if call.block().is_some() {
        out.push(None);
    }
    out
}

/// `node.body.child_nodes` for a `def` body (`None` = not a `send`).
fn body_children<'pr>(body: &Node<'pr>) -> Vec<Option<CallNode<'pr>>> {
    if let Some(statements) = body.as_statements_node() {
        let list: Vec<Node<'pr>> = statements.body().iter().collect();
        return match list.as_slice() {
            [single] => single.as_call_node().map_or_else(|| vec![None], |call| call_children(&call)),
            _ => list.iter().map(as_send).collect(),
        };
    }
    if let Some(begin) = body.as_begin_node() {
        // `(rescue body (resbody ...) else)` / `(ensure ...)`: the body is a
        // child, followed by clauses that are never `send`s.
        let mut out = Vec::new();
        if let Some(statements) = begin.statements() {
            let list: Vec<Node<'pr>> = statements.body().iter().collect();
            match list.as_slice() {
                [single] => out.push(as_send(single)),
                _ => out.push(None),
            }
        }
        if begin.rescue_clause().is_some() {
            out.push(None);
        }
        if begin.else_clause().is_some() {
            out.push(None);
        }
        if begin.ensure_clause().is_some() {
            out.push(None);
        }
        return out;
    }
    vec![None]
}

/// `send_nodes_from_change_table_block`.
fn send_nodes_from_change_table_block<'pr>(body: &Node<'pr>) -> Vec<CallNode<'pr>> {
    if let Some(statements) = body.as_statements_node() {
        let list: Vec<Node<'pr>> = statements.body().iter().collect();
        if let [single] = list.as_slice() {
            if let Some(call) = as_send(single) {
                return vec![call];
            }
        }
    }
    body_children(body).into_iter().flatten().collect()
}

/// `include_bulk_options?`: the second argument is a hash with a `bulk:` key.
fn include_bulk_options(call: &CallNode<'_>) -> bool {
    let arguments = arguments_of(call);
    let Some(options) = arguments.get(1) else { return false };
    let elements = if let Some(hash) = options.as_hash_node() {
        hash.elements()
    } else if let Some(hash) = options.as_keyword_hash_node() {
        hash.elements()
    } else {
        return false;
    };
    elements.iter().any(|element| {
        element
            .as_assoc_node()
            .and_then(|assoc| assoc.key().as_symbol_node().map(|key| key.unescaped() == b"bulk"))
            .unwrap_or(false)
    })
}

fn arguments_of<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut out: Vec<Node<'pr>> =
        call.arguments().map_or_else(Vec::new, |arguments| arguments.arguments().iter().collect());
    if let Some(block) = call.block() {
        if block.as_block_argument_node().is_some() {
            out.push(block);
        }
    }
    out
}

fn first_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    arguments_of(call).into_iter().next()
}

fn is_hash(node: &Node<'_>) -> bool {
    node.as_hash_node().is_some() || node.as_keyword_hash_node().is_some()
}

/// `BasicLiteralNode#value.to_s` for the literal kinds that include it
/// (`str`/`dstr`, `sym`, `int`, `float`, `rational`, `complex`), using the
/// source text for the numeric ones.
fn literal_value(node: &Node<'_>, ctx: &Context<'_>) -> Option<String> {
    if let Some(string) = node.as_string_node() {
        return Some(String::from_utf8_lossy(string.unescaped()).into_owned());
    }
    if let Some(symbol) = node.as_symbol_node() {
        return Some(String::from_utf8_lossy(symbol.unescaped()).into_owned());
    }
    if let Some(string) = node.as_interpolated_string_node() {
        // `DstrNode#value`: children's values, or their source when they have
        // none (interpolations).
        let mut out = String::new();
        for part in string.parts().iter() {
            if let Some(text) = part.as_string_node() {
                out.push_str(&String::from_utf8_lossy(text.unescaped()));
            } else if let Some(nested) = literal_value(&part, ctx) {
                out.push_str(&nested);
            } else if let Some(variable) = part.as_embedded_variable_node() {
                out.push_str(&String::from_utf8_lossy(ctx.text(variable.variable().span())));
            } else {
                out.push_str(&String::from_utf8_lossy(ctx.text(part.span())));
            }
        }
        return Some(out);
    }
    if node.as_integer_node().is_some()
        || node.as_float_node().is_some()
        || node.as_rational_node().is_some()
        || node.as_imaginary_node().is_some()
    {
        return Some(String::from_utf8_lossy(ctx.text(node.span())).replace('_', ""));
    }
    None
}

/// `value.to_s` used for comparing table names between statements.
fn table_text(node: &Node<'_>) -> Option<String> {
    // The recorder compares `value.to_s`, which needs no source access for
    // strings and symbols; other literals compare by their source text.
    if let Some(string) = node.as_string_node() {
        return Some(String::from_utf8_lossy(string.unescaped()).into_owned());
    }
    if let Some(symbol) = node.as_symbol_node() {
        return Some(String::from_utf8_lossy(symbol.unescaped()).into_owned());
    }
    None
}

/// `Config#target_rails_version`: `AllCops/TargetRailsVersion` when set.
fn target_rails_version(options: &RuleOptions) -> f64 {
    match options.peer("AllCops", "TargetRailsVersion") {
        Some(OptionValue::Str(text)) => text.trim().parse().unwrap_or(DEFAULT_RAILS_VERSION),
        Some(value) => value.as_float().unwrap_or(DEFAULT_RAILS_VERSION),
        None => DEFAULT_RAILS_VERSION,
    }
}

/// `DatabaseTypeResolvable#database`:
/// `cop_config['Database'] || database_from_yaml || database_from_env`.
fn resolve_database(options: &RuleOptions) -> Option<Database> {
    if let Some(configured) = options.get("Database").and_then(OptionValue::as_str) {
        return match configured {
            "mysql" => Some(Database::Mysql),
            "postgresql" => Some(Database::Postgresql),
            _ => None,
        };
    }
    database_from_yaml().or_else(database_from_env)
}

fn database_from_yaml() -> Option<Database> {
    let adapter = database_adapter()?;
    match adapter.as_str() {
        "mysql2" | "trilogy" => Some(Database::Mysql),
        "postgresql" | "postgis" => Some(Database::Postgresql),
        _ => None,
    }
}

fn database_from_env() -> Option<Database> {
    let url = std::env::var("DATABASE_URL").ok()?;
    let scheme = url.split_once("://")?.0;
    match scheme {
        "mysql2" | "trilogy" => Some(Database::Mysql),
        "postgres" | "postgresql" => Some(Database::Postgresql),
        _ => None,
    }
}

/// `database_yaml['adapter'] || database_yaml('shared')&.dig('adapter') ||
/// database_yaml.first.last['adapter']`, or `None` without a usable
/// `development` entry in `config/database.yml`.
fn database_adapter() -> Option<String> {
    let text = std::fs::read_to_string("config/database.yml").ok()?;
    let yaml = config::parse_yaml(&text).ok()??;
    let root = yaml.as_mapping()?;
    let development = root.get_mapping("development")?;
    let adapter = |mapping: &config::Mapping| mapping.get_str("adapter").map(str::to_owned);
    adapter(development)
        .or_else(|| root.get_mapping("shared").and_then(adapter))
        .or_else(|| development.iter().next().and_then(|(_, value)| value.as_mapping()).and_then(adapter))
}


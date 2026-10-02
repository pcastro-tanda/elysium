//! `Rails/NotNullColumn`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/not_null_column.rb` (with its
//! `DatabaseTypeResolvable` mixin).

use config::{parse_yaml, Mapping, YamlValue};
use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, OptionValue,
    Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Do not add a NOT NULL column without a default value.";
const DATABASE_YAML: &str = "config/database.yml";

/// Checks for `add_column` calls with a NOT NULL constraint without a default value.
#[derive(Debug, Clone)]
pub struct NotNullColumn {
    /// `cop_config['Database']`, when set.
    configured_database: Option<String>,
    /// Whether `database == MYSQL`, resolved on first use.
    mysql: Option<bool>,
}

impl Rule for NotNullColumn {
    const META: RuleMeta = RuleMeta {
        name: "Rails/NotNullColumn",
        department: Department::Rails,
        summary: "Do not add a NOT NULL column without a default value to existing tables.",
        explanation: "Checks for add_column calls with a NOT NULL constraint without a default \
                      value.\n\nThis cop only applies when adding a column to an existing \
                      table, since existing records will not have a value for the new column. \
                      New tables can freely use NOT NULL columns without defaults, since there \
                      are no records that could violate the constraint.\n\n```ruby\n# bad\nadd_column \
                      :users, :name, :string, null: false\nadd_reference :products, :category, \
                      null: false\n\n# good\nadd_column :users, :name, :string, null: true\nadd_column \
                      :users, :name, :string, null: false, default: ''\nadd_reference :products, \
                      :category\nadd_reference :products, :category, null: false, default: 1\n\n# \
                      good (changing an existing column)\nchange_column :users, :name, :string, \
                      null: false\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "Database",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "The database in use, `mysql` or `postgresql`; inferred from \
                      `config/database.yml` or `DATABASE_URL` when unset.",
            },
            ConfigOption {
                name: "SupportedDatabases",
                default: ConfigDefault::StrList(&["mysql"]),
                allowed: &[],
                doc: "Databases the cop knows how to handle.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let configured_database = match options.get("Database") {
            Some(OptionValue::Str(database)) => Some(database.clone()),
            Some(OptionValue::Null | OptionValue::Bool(false)) | None => None,
            Some(other) => Some(format!("{other:?}")),
        };
        Ok(Self { configured_database, mysql: None })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        match name.as_slice() {
            b"add_column" | b"add_reference" => {
                if call.receiver().is_some() {
                    return;
                }
                let Some(arguments) = send_arguments(&call) else { return };
                if name.as_slice() == b"add_column" {
                    // `(send nil? :add_column _ _ $_ (hash $...))`.
                    if let [_, _, kind, options] = arguments.as_slice() {
                        if let Some(pairs) = hash_pairs(options) {
                            self.check_column(ctx, Some(kind), &pairs);
                        }
                    }
                } else if let [_, _, options] = arguments.as_slice() {
                    // `(send nil? :add_reference _ _ (hash $...))`.
                    if let Some(pairs) = hash_pairs(options) {
                        check_pairs(ctx, &pairs);
                    }
                }
            }
            b"change_table" if call.receiver().is_none() => self.check_change_table(ctx, &call),
            _ => {}
        }
    }
}

impl NotNullColumn {
    /// `check_change_table`:
    /// `(block (send nil? :change_table ...) (args (arg $_)) _)`.
    fn check_change_table(&mut self, ctx: &mut Context<'_>, call: &ruby_ast::node::CallNode<'_>) {
        let Some(block) = call.block().and_then(|block| block.as_block_node()) else { return };
        let Some(parameters) = block.parameters().and_then(|p| p.as_block_parameters_node()) else {
            return;
        };
        if !parameters.locals().is_empty() {
            return;
        }
        let Some(parameters) = parameters.parameters() else { return };
        let required: Vec<Node<'_>> = parameters.requireds().iter().collect();
        if parameters.optionals().iter().count() != 0
            || parameters.rest().is_some()
            || parameters.posts().iter().count() != 0
            || parameters.keywords().iter().count() != 0
            || parameters.keyword_rest().is_some()
            || parameters.block().is_some()
        {
            return;
        }
        let [table] = required.as_slice() else { return };
        let Some(table) = table.as_required_parameter_node() else { return };
        let table = table.name().as_slice().to_vec();

        let Some(body) = block.body() else { return };
        let Some(statements) = body.as_statements_node() else { return };
        for child in &statements.body() {
            let Some(child) = child.as_call_node() else { continue };
            // `(send (lvar $_) ...)`.
            let Some(receiver) = child.receiver().and_then(|r| r.as_local_variable_read_node())
            else {
                continue;
            };
            if receiver.name().as_slice() != table.as_slice() || child.is_safe_navigation() {
                continue;
            }
            let Some(arguments) = send_arguments(&child) else { continue };
            let method = child.name();
            if method.as_slice() == b"column" {
                // `(send (lvar $_) :column _ $_ (hash $...))`.
                if let [_, kind, options] = arguments.as_slice() {
                    if let Some(pairs) = hash_pairs(options) {
                        self.check_column(ctx, Some(kind), &pairs);
                    }
                }
            }
            // `(send (lvar $_) $_ _ (hash $...))`: the type is the method
            // name, a `Symbol` without `#value`.
            if let [_, options] = arguments.as_slice() {
                if let Some(pairs) = hash_pairs(options) {
                    self.check_column(ctx, None, &pairs);
                }
            }
            // `(send (lvar $_) :add_reference _ _ (hash $...))`.
            if method.as_slice() == b"add_reference" {
                if let [_, _, options] = arguments.as_slice() {
                    if let Some(pairs) = hash_pairs(options) {
                        check_pairs(ctx, &pairs);
                    }
                }
            }
        }
    }

    /// `check_column`; `kind` is `None` where the type is a bare `Symbol`.
    fn check_column(&mut self, ctx: &mut Context<'_>, kind: Option<&Node<'_>>, pairs: &[Node<'_>]) {
        let value = kind.and_then(|kind| {
            kind.as_symbol_node()
                .map(|symbol| symbol.unescaped().to_vec())
                .or_else(|| kind.as_string_node().map(|string| string.unescaped().to_vec()))
        });
        if let Some(value) = value {
            if value == b"virtual" {
                return;
            }
            if value == b"text" && self.is_mysql() {
                return;
            }
        }
        check_pairs(ctx, pairs);
    }

    /// `database == MYSQL`.
    fn is_mysql(&mut self) -> bool {
        if let Some(mysql) = self.mysql {
            return mysql;
        }
        let mysql = match &self.configured_database {
            Some(database) => database == "mysql",
            None => (database_from_yaml().or_else(database_from_env)).as_deref() == Some("mysql"),
        };
        self.mysql = Some(mysql);
        mysql
    }
}

/// The whitequark `send` arguments, a `&block` argument included; `None` for
/// a call with a literal block (a `block` node there).
fn send_arguments<'pr>(call: &ruby_ast::node::CallNode<'pr>) -> Option<Vec<Node<'pr>>> {
    let mut arguments: Vec<Node<'pr>> =
        call.arguments().map_or_else(Vec::new, |args| args.arguments().iter().collect());
    if let Some(block) = call.block() {
        block.as_block_argument_node()?;
        arguments.push(block);
    }
    Some(arguments)
}

/// `(hash $...)`: every child of a braced or braceless hash.
fn hash_pairs<'pr>(node: &Node<'pr>) -> Option<Vec<Node<'pr>>> {
    let elements = match node.kind() {
        NodeKind::HashNode => node.as_hash_node()?.elements(),
        NodeKind::KeywordHashNode => node.as_keyword_hash_node()?.elements(),
        _ => return None,
    };
    Some(elements.iter().collect())
}

/// `(pair (sym :name) ...)`'s value.
fn symbol_pair_value<'pr>(pair: &Node<'pr>, name: &[u8]) -> Option<Node<'pr>> {
    let pair = pair.as_assoc_node()?;
    let key = pair.key();
    (key.as_symbol_node()?.unescaped() == name).then(|| pair.value())
}

/// `check_pairs`.
fn check_pairs(ctx: &mut Context<'_>, pairs: &[Node<'_>]) {
    // `default_option?`: `(pair (sym :default) !nil)`.
    if pairs
        .iter()
        .any(|pair| symbol_pair_value(pair, b"default").is_some_and(|v| v.as_nil_node().is_none()))
    {
        return;
    }
    // `null_false?`: `(pair (sym :null) (false))`.
    let null_false = pairs
        .iter()
        .find(|pair| symbol_pair_value(pair, b"null").is_some_and(|v| v.as_false_node().is_some()));
    if let Some(null_false) = null_false {
        ctx.report(&NotNullColumn::META, null_false.span(), MSG);
    }
}

/// `database_from_yaml`: the development configuration's adapter.
fn database_from_yaml() -> Option<String> {
    let development = database_yaml("development")?;
    let adapter = adapter_of(&development)
        .or_else(|| database_yaml("shared").and_then(|shared| adapter_of(&shared)))
        .or_else(|| {
            development.iter().next().and_then(|(_, first)| match first {
                YamlValue::Mapping(first) => adapter_of(first),
                _ => None,
            })
        })?;
    match adapter.as_str() {
        "mysql2" | "trilogy" => Some("mysql".to_owned()),
        "postgresql" | "postgis" => Some("postgresql".to_owned()),
        _ => None,
    }
}

fn adapter_of(mapping: &Mapping) -> Option<String> {
    let adapter = mapping.get("adapter")?;
    adapter
        .is_truthy()
        .then(|| adapter.as_str().map_or_else(|| format!("{adapter:?}"), str::to_owned))
}

/// `database_yaml(environment)`: the environment's hash in `config/database.yml`.
fn database_yaml(environment: &str) -> Option<Mapping> {
    let text = std::fs::read_to_string(DATABASE_YAML).ok()?;
    let YamlValue::Mapping(yaml) = parse_yaml(&text).ok()?? else { return None };
    match yaml.get(environment)? {
        YamlValue::Mapping(config) => Some(config.clone()),
        _ => None,
    }
}

/// `database_from_env`.
fn database_from_env() -> Option<String> {
    let url = std::env::var("DATABASE_URL").ok()?;
    if url.starts_with("mysql2://") || url.starts_with("trilogy://") {
        Some("mysql".to_owned())
    } else if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        Some("postgresql".to_owned())
    } else {
        None
    }
}

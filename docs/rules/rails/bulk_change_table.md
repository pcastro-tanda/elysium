# Rails/BulkChangeTable

Check whether alter queries are combinable.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks whether alter queries are combinable. If combinable queries are detected, it suggests to you to use `change_table` with `bulk: true` instead. This option causes the migration to generate a single ALTER TABLE statement combining multiple column alterations.

The `bulk` option is only supported on the MySQL and the PostgreSQL (5.2 later) adapter; thus it will automatically detect an adapter from `development` environment in `config/database.yml` or the environment variable `DATABASE_URL` when the `Database` option is not set. If the adapter is not `mysql2`, `trilogy`, `postgresql`, or `postgis`, this Cop ignores offenses.

```ruby
# bad
def change
  add_column :users, :name, :string, null: false
  add_column :users, :nickname, :string
end

# good
def change
  change_table :users, bulk: true do |t|
    t.string :name, null: false
    t.string :nickname
  end
end
```

```ruby
# bad
def change
  change_table :users do |t|
    t.string :name, null: false
    t.string :nickname
  end
end

# good
def change
  change_table :users, bulk: true do |t|
    t.string :name, null: false
    t.string :nickname
  end
end

# good
# When you don't want to combine alter queries.
def change
  change_table :users, bulk: false do |t|
    t.string :name, null: false
    t.string :nickname
  end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Database | `nil` |  | Database adapter (`mysql` or `postgresql`); detected from `config/database.yml` or `DATABASE_URL` when unset. |
| SupportedDatabases | `mysql`, `postgresql` |  | Databases that support bulk alter. |

## Blind spots

`config/database.yml` and `DATABASE_URL` are read relative to the working directory, as RuboCop does.

# Rails/NotNullColumn

Do not add a NOT NULL column without a default value to existing tables.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for add_column calls with a NOT NULL constraint without a default value.

This cop only applies when adding a column to an existing table, since existing records will not have a value for the new column. New tables can freely use NOT NULL columns without defaults, since there are no records that could violate the constraint.

```ruby
# bad
add_column :users, :name, :string, null: false
add_reference :products, :category, null: false

# good
add_column :users, :name, :string, null: true
add_column :users, :name, :string, null: false, default: ''
add_reference :products, :category
add_reference :products, :category, null: false, default: 1

# good (changing an existing column)
change_column :users, :name, :string, null: false
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Database | `nil` |  | The database in use, `mysql` or `postgresql`; inferred from `config/database.yml` or `DATABASE_URL` when unset. |
| SupportedDatabases | `mysql` |  | Databases the cop knows how to handle. |

## Blind spots

None recorded.

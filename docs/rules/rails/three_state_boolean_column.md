# Rails/ThreeStateBooleanColumn

Add a default value and a `NOT NULL` constraint to boolean columns.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Enforces that boolean columns are created with default values (`false` or `true`) and `NOT NULL` constraint.

```ruby
# bad
add_column :users, :active, :boolean
t.column :active, :boolean
t.boolean :active

# good
add_column :users, :active, :boolean, default: true, null: false
t.column :active, :boolean, default: true, null: false
t.boolean :active, default: true, null: false
```

## Options

This rule has no options.

## Blind spots

None recorded.

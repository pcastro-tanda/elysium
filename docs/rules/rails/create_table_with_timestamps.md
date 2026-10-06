# Rails/CreateTableWithTimestamps

Checks the migration for which timestamps are not included when creating a new table.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks the migration for which timestamps are not included when creating a new table. In many cases, timestamps are useful information and should be added.

The `id: false` option is respected: a table without a primary key is usually a join table.

```ruby
# bad
create_table :users

create_table :users do |t|
  t.string :name
  t.string :email
end

# good
create_table :users do |t|
  t.string :name
  t.string :email

  t.timestamps
end
```

## Options

This rule has no options.

## Blind spots

None recorded.

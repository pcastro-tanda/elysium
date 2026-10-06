# Rails/DangerousColumnNames

Avoid dangerous column names.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Avoid dangerous column names.

Some column names are considered dangerous because they would overwrite methods already defined.

```ruby
# bad
add_column :users, :save

# good
add_column :users, :saved
```

## Options

This rule has no options.

## Blind spots

None recorded.

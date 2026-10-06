# Rails/AddColumnIndex

Rails migrations don't make use of a given `index` key, but also doesn't given an error when it's used, so it makes it seem like an index might be used.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for migrations using `add_column` that have an `index` key. `add_column` does not accept `index`, but also does not raise an error for extra keys, so it is possible to mistakenly add the key without realizing it will not actually add an index.

```ruby
# bad (will not add an index)
add_column :table, :column, :integer, index: true

# good
add_column :table, :column, :integer
add_index :table, :column
```

## Options

This rule has no options.

## Blind spots

None recorded.

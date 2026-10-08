# Rails/ArelStar

Enforces `Arel.star` instead of `"*"` for expanded columns.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Prevents usage of `"*"` on an Arel::Table column reference.

Using `arel_table["\*"]` causes the outputted string to be a literal quoted asterisk (e.g. `my_model`.`*`). This causes the database to look for a column named `\*` (or `"*"`) as opposed to expanding the column list as one would likely expect.

This cop's autocorrection is unsafe because it turns a quoted `\*` into an SQL `*`, unquoted. `\*` is a valid column name in certain databases supported by Rails, and even though it is usually a mistake, it might denote legitimate access to a column named `*`.

```ruby
# bad
MyTable.arel_table["*"]

# good
MyTable.arel_table[Arel.star]
```

## Options

This rule has no options.

## Blind spots

None recorded.

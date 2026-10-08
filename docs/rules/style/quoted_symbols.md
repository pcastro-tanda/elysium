# Style/QuotedSymbols

Use a consistent style for quoted symbols.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

By default uses the same configuration as `Style/StringLiterals`; if that
cop is not enabled, the default `EnforcedStyle` is `single_quotes`.

String interpolation is always kept in double quotes.

```ruby
# EnforcedStyle: same_as_string_literals (default) / single_quotes
# bad
:"abc-def"

# good
:'abc-def'
:"#{str}"
:"a\'b"
```

```ruby
# EnforcedStyle: double_quotes
# bad
:'abc-def'

# good
:"abc-def"
:"#{str}"
:"a\'b"
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `same_as_string_literals` | `same_as_string_literals`, `single_quotes`, `double_quotes` | Whether to use the same style as `Style/StringLiterals`, always single quotes, or always double quotes. |

## Blind spots

None recorded.

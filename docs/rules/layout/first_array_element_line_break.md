# Layout/FirstArrayElementLineBreak

Checks for a line break before the first element in a multi-line array.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
[ :a,
  :b]

# good
[
  :a,
  :b]

# good
[:a, :b]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowImplicitArrayLiterals | false |  | Whether an implicit (bracket-less) array literal, e.g. the right-hand side of a multiple assignment, is exempt from this check. |
| AllowMultilineFinalElement | false |  | Whether the last element of the array is allowed to start a new, multi-line element without triggering this cop. |

## Blind spots

None recorded.

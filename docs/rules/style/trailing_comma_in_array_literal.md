# Style/TrailingCommaInArrayLiteral

Checks for trailing comma in array literals.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

The configuration options are:

* `consistent_comma`: Requires a comma after the last item of all non-empty,
  multiline array literals.
* `comma`: Requires a comma after the last item in an array, but only when
  each item is on its own line.
* `diff_comma`: Requires a comma after the last item in an array, but only
  when that item is followed by an immediate newline, even if there is an
  inline comment on the same line.
* `no_comma` (default): Does not require a comma after the last item in an
  array.

```ruby
# EnforcedStyleForMultiline: no_comma (default)

# bad
a = [1, 2,]

# good
a = [
  1,
  2
]
```

```ruby
# EnforcedStyleForMultiline: comma

# bad
a = [
  1, 2,
  3,
]

# good
a = [
  1, 2,
  3
]
```

```ruby
# EnforcedStyleForMultiline: consistent_comma

# good
a = [
  1, 2,
  3,
]

# good
a = [
  1, 2, 3,
]
```

```ruby
# EnforcedStyleForMultiline: diff_comma

# bad
a = [1, 2,
     3, 4,]

# good
a = [1, 2,
     3, 4]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyleForMultiline | `no_comma` | `comma`, `consistent_comma`, `diff_comma`, `no_comma` | The comma style to require for multiline array literals. |

## Blind spots

Heredoc detection only follows direct heredoc string literals and method
chains rooted at a heredoc receiver or ending in a heredoc argument (RuboCop's
`heredoc_send?`); a heredoc nested inside a hash-literal array element's value
(RuboCop's `pair`/`hash` case of `heredoc?`) is not recognised, which can only
turn a false positive into a rarer false negative (a trailing-comma-in-comma
misdetection next to such a value), consistent with the false-negatives-over-
false-positives policy.

# Style/TrailingCommaInArguments

Checks for trailing comma in argument lists.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

Regardless of style, trailing commas are not allowed in single-line method
calls.

```ruby
# EnforcedStyleForMultiline: consistent_comma
# bad
method(1, 2,)

# good
method(1, 2)

# good
method(
  1, 2,
  3,
)

# good
method(
  1, 2, 3,
)

# good
method(
  1,
  2,
)
```

```ruby
# EnforcedStyleForMultiline: comma
# bad
method(1, 2,)

# good
method(1, 2)

# bad
method(
  1, 2,
  3,
)

# good
method(
  1, 2,
  3
)

# bad
method(
  1, 2, 3,
)

# good
method(
  1, 2, 3
)

# good
method(
  1,
  2,
)
```

```ruby
# EnforcedStyleForMultiline: diff_comma
# bad
method(1, 2,)

# good
method(1, 2)

# good
method(
  1, 2,
  3,
)

# good
method(
  1, 2, 3,
)

# good
method(
  1,
  2,
)

# bad
method(1, [
  2,
],)

# good
method(1, [
  2,
])

# bad
object[1, 2,
       3, 4,]

# good
object[1, 2,
       3, 4]
```

```ruby
# EnforcedStyleForMultiline: no_comma (default)
# bad
method(1, 2,)

# bad
object[1, 2,]

# good
method(1, 2)

# good
object[1, 2]

# good
method(
  1,
  2
)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyleForMultiline | `no_comma` | `comma`, `consistent_comma`, `diff_comma`, `no_comma` | Trailing comma style for multiline parenthesized/`[]` method calls. |

## Blind spots

Only parenthesized calls and index reads (`foo(...)`/`foo[...]`) are checked,
matching RuboCop's own `on_send` guard; unparenthesized calls, `super`,
`yield`, and index *assignment* (`foo[...] = ...`, a different method name in
Prism) are never checked, since RuboCop doesn't check them either.

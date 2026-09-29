# Style/TrailingUnderscoreVariable

Checks for the usage of unneeded trailing underscores at the end of parallel variable assignment.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for extra underscores in variable assignment.

```ruby
# bad
a, b, _ = foo
a, b, _, = foo
a, _, _ = foo
a, _, _, = foo

# good
a, b, = foo
a, = foo
*a, b, _ = foo
# => We need to know to not include 2 variables in a
a, *b, _ = foo
# => The correction `a, *b, = foo` is a syntax error
```

With `AllowNamedUnderscoreVariables: true` (default):

```ruby
# good
a, b, _something = foo
```

With `AllowNamedUnderscoreVariables: false`:

```ruby
# bad
a, b, _something = foo
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowNamedUnderscoreVariables | true |  | Whether a splat/local variable target named with a leading underscore other than a bare `_` (e.g. `_foo`) also counts as a trailing underscore variable. |

## Blind spots

None recorded.

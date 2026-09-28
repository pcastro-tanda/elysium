# Style/TernaryParentheses

Checks for use of parentheses around ternary conditions.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for the presence of parentheses around ternary
conditions. It is configurable to enforce inclusion or omission of
parentheses using `EnforcedStyle`. Omission is only enforced when
removing the parentheses won't cause a different behavior.

`AllowSafeAssignment` option for safe assignment.
By safe assignment we mean putting parentheses around
an assignment to indicate "I know I'm using an assignment
as a condition. It's not a mistake."

```ruby
# EnforcedStyle: require_no_parentheses (default)
# bad
foo = (bar?) ? a : b
foo = (bar.baz?) ? a : b
foo = (bar && baz) ? a : b

# good
foo = bar? ? a : b
foo = bar.baz? ? a : b
foo = bar && baz ? a : b
```

```ruby
# EnforcedStyle: require_parentheses
# bad
foo = bar? ? a : b
foo = bar.baz? ? a : b
foo = bar && baz ? a : b

# good
foo = (bar?) ? a : b
foo = (bar.baz?) ? a : b
foo = (bar && baz) ? a : b
```

```ruby
# EnforcedStyle: require_parentheses_when_complex
# bad
foo = (bar?) ? a : b
foo = (bar.baz?) ? a : b
foo = bar && baz ? a : b

# good
foo = bar? ? a : b
foo = bar.baz? ? a : b
foo = (bar && baz) ? a : b
```

```ruby
# AllowSafeAssignment: true (default)
# good
foo = (bar = baz) ? a : b
```

```ruby
# AllowSafeAssignment: false
# bad
foo = (bar = baz) ? a : b
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `require_no_parentheses` | `require_parentheses`, `require_no_parentheses`, `require_parentheses_when_complex` | Whether ternary conditions must, must not, or must (only when complex) be wrapped in parentheses. |
| AllowSafeAssignment | true |  | Whether an assignment wrapped in parentheses (`(bar = baz) ? a : b`) is allowed as a deliberate "I know I'm using an assignment" marker. |

## Blind spots

None recorded.

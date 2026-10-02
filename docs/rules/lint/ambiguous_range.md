# Lint/AmbiguousRange

Checks for ranges with ambiguous boundaries.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for ambiguous ranges.

Ranges have quite low precedence, which leads to unexpected behavior when using a range with other operators. This cop avoids that by making ranges explicit by requiring parenthesis around complex range boundaries (anything that is not a literal: numerics, strings, symbols, etc.).

This cop can be configured with `RequireParenthesesForMethodChains` in order to specify whether method chains (including `self.foo`) should be wrapped in parens by this cop.

NOTE: Regardless of this configuration, if a method receiver is a basic literal value, it will be wrapped in order to prevent the ambiguity of `1..2.to_a`.

```ruby
# bad
x || 1..2
x - 1..2
(x || 1..2)
x || 1..y || 2
1..2.to_a

# good, unambiguous
1..2
'a'..'z'
:bar..:baz
MyClass::MIN..MyClass::MAX
@min..@max
a..b
-a..b

# good, ambiguity removed
x || (1..2)
(x - 1)..2
(x || 1)..2
(x || 1)..(y || 2)
(1..2).to_a
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| RequireParenthesesForMethodChains | false |  | Require parentheses for method chain boundaries (including `self.foo`). |

## Blind spots

None recorded.

# Layout/SpaceInLambdaLiteral

Checks for spaces between `->` and opening parameter parenthesis (`(`) in lambda literals.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad, EnforcedStyle: require_no_space (default)
a = -> (x, y) { x + y }

# good, EnforcedStyle: require_no_space (default)
a = ->(x, y) { x + y }

# bad, EnforcedStyle: require_space
a = ->(x, y) { x + y }

# good, EnforcedStyle: require_space
a = -> (x, y) { x + y }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `require_no_space` | `require_no_space`, `require_space` | Which spacing style to enforce between `->` and `(` in lambda literals. |

## Blind spots

None recorded.

# Style/ClassCheck

Enforces consistent use of `Object#is_a?` or `Object#kind_of?`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: is_a? (default)
# bad
var.kind_of?(Date)
var.kind_of?(Integer)

# good
var.is_a?(Date)
var.is_a?(Integer)
```

```ruby
# EnforcedStyle: kind_of?
# bad
var.is_a?(Time)
var.is_a?(String)

# good
var.kind_of?(Time)
var.kind_of?(String)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `is_a?` | `is_a?`, `kind_of?` | Whether to prefer `Object#is_a?` or `Object#kind_of?`. |

## Blind spots

None recorded.

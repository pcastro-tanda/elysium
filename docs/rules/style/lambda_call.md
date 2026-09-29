# Style/LambdaCall

Use lambda.call(...) instead of lambda.(...).

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: call (default)

# bad
lambda.(x, y)

# good
lambda.call(x, y)
```

```ruby
# EnforcedStyle: braces

# bad
lambda.call(x, y)

# good
lambda.(x, y)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `call` | `call`, `braces` | Whether to prefer `lambda.call(...)` or `lambda.(...)`. |

## Blind spots

None recorded.

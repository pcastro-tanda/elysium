# Style/NilComparison

Checks for comparison of something with nil using `==` and `nil?`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for comparison of something with nil using `==` and `nil?`. Enforcing a consistent style (either the `nil?` predicate or `==` comparison) improves readability.

```ruby
# EnforcedStyle: predicate (default)

# bad
if x == nil
end

# good
if x.nil?
end
```

```ruby
# EnforcedStyle: comparison

# bad
if x.nil?
end

# good
if x == nil
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `predicate` | `predicate`, `comparison` | Whether to prefer the `nil?` predicate or `==`/`===` comparison against `nil`. |

## Blind spots

None recorded.

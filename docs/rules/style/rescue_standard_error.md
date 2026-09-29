# Style/RescueStandardError

Avoid rescuing without specifying an error class.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for rescuing `StandardError`. There are two supported
styles `implicit` and `explicit`. This cop will not register an offense
if any error other than `StandardError` is specified.

```ruby
# EnforcedStyle: explicit (default)
# `explicit` will enforce using `rescue StandardError`
# instead of `rescue`.

# bad
begin
  foo
rescue
  bar
end

# good
begin
  foo
rescue StandardError
  bar
end

# EnforcedStyle: implicit
# `implicit` will enforce using `rescue` instead of
# `rescue StandardError`.

# bad
begin
  foo
rescue StandardError
  bar
end

# good
begin
  foo
rescue
  bar
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `explicit` | `implicit`, `explicit` | Whether to enforce `rescue` or `rescue StandardError` for the bare case. |

## Blind spots

None recorded.

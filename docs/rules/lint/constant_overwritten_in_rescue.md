# Lint/ConstantOverwrittenInRescue

Checks for overwriting an exception with an exception result by using `rescue =>`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for overwriting an exception with an exception result by using ``rescue =>``.

You intended to write as `rescue StandardError`. However, you have written `rescue => StandardError`. In that case, the result of `rescue` will overwrite `StandardError`.

```ruby
# bad
begin
  something
rescue => StandardError
end

# good
begin
  something
rescue StandardError
end
```

## Options

This rule has no options.

## Blind spots

None recorded.

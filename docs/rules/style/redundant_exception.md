# Style/RedundantException

Checks for an obsolete RuntimeException argument in raise/fail.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for `RuntimeError` as the argument of `raise`/`fail`.

# Examples

```ruby
# bad
raise RuntimeError, 'message'
raise RuntimeError.new('message')

# good
raise 'message'

# bad - message is not a string
raise RuntimeError, Object.new
raise RuntimeError.new(Object.new)

# good
raise Object.new.to_s
```

## Options

This rule has no options.

## Blind spots

None recorded.

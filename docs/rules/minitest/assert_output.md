# Minitest/AssertOutput

This cop checks for opportunities to use `assert_output`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for opportunities to use `assert_output`.

```ruby
# bad
$stdout = StringIO.new
puts object.method
$stdout.rewind
assert_match expected, $stdout.read

# good
assert_output(expected) { puts object.method }
```

## Options

This rule has no options.

## Blind spots

None recorded.

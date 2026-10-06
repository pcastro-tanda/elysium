# Minitest/AssertMatch

This cop enforces the test to use `assert_match` instead of using `assert(matcher.match(object))`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the test to use `assert_match` instead of using `assert(matcher.match(string))`.

```ruby
# bad
assert(matcher.match(string))
assert(matcher.match?(string))
assert(matcher =~ string)
assert_operator(matcher, :=~, string)
assert(matcher.match(string), 'message')

# good
assert_match(matcher, string)
assert_match(matcher, string, 'message')
```

## Options

This rule has no options.

## Blind spots

None recorded.

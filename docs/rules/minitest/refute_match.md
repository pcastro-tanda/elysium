# Minitest/RefuteMatch

This cop enforces the test to use `refute_match` instead of using `refute(matcher.match(object))`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the test to use `refute_match` instead of using `refute(matcher.match(string))`.

```ruby
# bad
refute(matcher.match(string))
refute(matcher.match?(string))
refute(matcher =~ string)
refute_operator(matcher, :=~, string)
assert_operator(matcher, :!~, string)
refute(matcher.match(string), 'message')

# good
refute_match(matcher, string)
refute_match(matcher, string, 'message')
```

## Options

This rule has no options.

## Blind spots

None recorded.

# Minitest/Focus

Checks for focused tests.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces tests are not focused.

```ruby
# bad
focus test 'foo' do
end

# bad
focus
test 'foo' do
end

# good
test 'foo' do
end
```

## Options

This rule has no options.

## Blind spots

None recorded.

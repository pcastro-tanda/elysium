# Minitest/AssertPathExists

This cop enforces the test to use `assert_path_exists` instead of using `assert(File.exist?(path))`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the test to use `assert_path_exists` instead of using `assert(File.exist?(path))`.

```ruby
# bad
assert(File.exist?(path))
assert(File.exist?(path), 'message')

# good
assert_path_exists(path)
assert_path_exists(path, 'message')
```

## Options

This rule has no options.

## Blind spots

None recorded.

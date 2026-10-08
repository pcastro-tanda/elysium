# Minitest/RefutePathExists

This cop enforces the test to use `refute_path_exists` instead of using `refute(File.exist?(path))`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the test to use `refute_path_exists` instead of using `refute(File.exist?(path))`.

```ruby
# bad
refute(File.exist?(path))
refute(File.exist?(path), 'message')

# good
refute_path_exists(path)
refute_path_exists(path, 'message')
```

## Options

This rule has no options.

## Blind spots

None recorded.

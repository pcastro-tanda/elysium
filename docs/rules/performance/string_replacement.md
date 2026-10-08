# Performance/StringReplacement

Use `tr` instead of `gsub` when you are replacing the same number of characters. Use `delete` instead of `gsub` when you are deleting characters.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `gsub` can be replaced by `tr` or `delete`.

```ruby
# bad
'abc'.gsub('b', 'd')
'abc'.gsub('a', '')
'abc'.gsub(/a/, 'd')
'abc'.gsub!('a', 'd')

# good
'abc'.gsub(/.*/, 'a')
'abc'.gsub(/a+/, 'd')
'abc'.tr('b', 'd')
'a b c'.delete(' ')
```

## Options

This rule has no options.

## Blind spots

None recorded.

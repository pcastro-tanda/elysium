# Performance/StringInclude

Use `String#include?` instead of a regex match with literal-only pattern.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies unnecessary use of a regex where `String#include?` would suffice.

```ruby
# bad
str.match?(/ab/)
/ab/.match?(str)
str =~ /ab/
/ab/ =~ str
str.match(/ab/)
/ab/.match(str)
/ab/ === str

# good
str.include?('ab')
```

## Options

This rule has no options.

## Blind spots

None recorded.

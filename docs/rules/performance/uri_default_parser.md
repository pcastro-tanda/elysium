# Performance/UriDefaultParser

Use `URI::DEFAULT_PARSER` instead of `URI::Parser.new`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `URI::Parser.new` can be replaced by `URI::DEFAULT_PARSER`.

```ruby
# bad
URI::Parser.new

# good
URI::DEFAULT_PARSER
```

## Options

This rule has no options.

## Blind spots

None recorded.

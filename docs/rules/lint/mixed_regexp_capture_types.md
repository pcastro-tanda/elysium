# Lint/MixedRegexpCaptureTypes

Checks for regexp literals that mix named and numbered captures.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Do not mix named captures and numbered captures in a `Regexp` literal
because numbered capture is ignored if they're mixed.
Replace numbered captures with non-capturing groupings or
named captures.

```ruby
# bad
/(?<foo>FOO)(BAR)/

# good
/(?<foo>FOO)(?<bar>BAR)/

# good
/(?<foo>FOO)(?:BAR)/

# good
/(FOO)(BAR)/
```

## Options

This rule has no options.

## Blind spots

RuboCop's `regexp_parser`-based capture scan silently yields no captures at all for a pattern it fails to parse (`rescue StandardError`); this port's simpler scanner never fails to parse and always finds whatever named/ numbered captures its grammar recognizes. No known fixture distinguishes the two.

# Style/RedundantFreeze

Checks usages of Object#freeze on immutable objects.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of `Object#freeze` on immutable objects.

NOTE: `Regexp` and `Range` literals are frozen objects since Ruby 3.0.

NOTE: From Ruby 3.0, this cop allows explicit freezing of interpolated
string literals when `# frozen-string-literal: true` is used.

```ruby
# bad
CONST = 1.freeze

# good
CONST = 1
```

## Options

This rule has no options.

## Blind spots

None recorded.

# Layout/SpaceAfterColon

Use spaces after colons.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for colon (`:`) not followed by some kind of space.
N.B. this cop does not handle spaces after a ternary operator, which are
instead handled by `Layout/SpaceAroundOperators`.

```ruby
# bad
def f(a:, b:2); {a:3}; end

# good
def f(a:, b: 2); {a: 3}; end
```

## Options

This rule has no options.

## Blind spots

None recorded.

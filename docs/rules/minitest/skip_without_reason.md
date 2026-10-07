# Minitest/SkipWithoutReason

Checks for skipped tests missing the skipping reason.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for skipped tests missing the skipping reason.

```ruby
# bad
skip
skip('')

# bad
if condition?
  skip
else
  skip
end

# good
skip("Reason why the test was skipped")

# good
skip if condition?
```

## Options

This rule has no options.

## Blind spots

None recorded.

# Layout/ConditionPosition

Checks for condition placed in a confusing position relative to the keyword.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

A multi-line `if`/`unless`/`while`/`until` reads oddly when its condition
sits on its own line below the keyword:

```ruby
# bad
if
  some_condition
  do_something
end

# good
if some_condition
  do_something
end
```

The fix moves the condition up onto the keyword's own line.

## Options

This rule has no options.

## Blind spots

None recorded.

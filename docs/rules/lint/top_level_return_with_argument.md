# Lint/TopLevelReturnWithArgument

Checks for top level return with arguments.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

If there is a top-level return statement with an argument, then the argument is always ignored. This is detected automatically since Ruby 2.7.

```ruby
# bad
return 1

# good
return
```

## Options

This rule has no options.

## Blind spots

None recorded.

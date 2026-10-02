# Layout/FirstMethodArgumentLineBreak

Checks for a line break before the first argument in a multi-line method call.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
method(foo, bar,
  baz)

# good
method(
  foo, bar,
  baz)

# ignored
method foo, bar,
  baz
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowMultilineFinalElement | false |  | Whether the last argument is allowed to start a new, multi-line element without triggering this cop. |
| AllowedMethods | `[]` |  | Method names that are never checked. The deprecated `IgnoredMethods`/`ExcludedMethods` aliases are merged in too. |

## Blind spots

None recorded.

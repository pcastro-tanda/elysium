# Layout/FirstMethodParameterLineBreak

Checks for a line break before the first parameter in a multi-line method parameter definition.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
def method(foo, bar,
    baz)
  do_something
end

# good
def method(
    foo, bar,
    baz)
  do_something
end

# ignored
def method foo,
    bar
  do_something
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowMultilineFinalElement | false |  | Whether the last parameter is allowed to start a new, multi-line element without triggering this cop. |

## Blind spots

None recorded.

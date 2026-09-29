# Style/EmptyBlockParameter

Omit pipes for empty block parameters.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for pipes for empty block parameters. Pipes for empty block parameters do not cause syntax errors, but they are redundant.

```ruby
# bad
a do ||
do_something
end

# bad
a { || do_something }

# good
a do
end

# good
a { do_something }
```

## Options

This rule has no options.

## Blind spots

None recorded.

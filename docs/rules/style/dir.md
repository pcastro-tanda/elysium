# Style/Dir

Use the `__dir__` method to retrieve the canonicalized absolute path to the current file.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for places where the `#__dir__` method can replace more complex constructs to retrieve a canonicalized absolute path to the current file.

```ruby
# bad
path = File.expand_path(File.dirname(__FILE__))

# bad
path = File.dirname(File.realpath(__FILE__))

# good
path = __dir__
```

## Options

This rule has no options.

## Blind spots

None recorded.

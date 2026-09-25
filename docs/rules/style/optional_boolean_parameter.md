# Style/OptionalBooleanParameter

Checks for places where keyword arguments can be used instead of boolean arguments when defining methods.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

`respond_to_missing?` method is allowed by default. These are customizable
with `AllowedMethods` option.

```ruby
# bad
def some_method(bar = false)
  puts bar
end

# bad - common hack before keyword args were introduced
def some_method(options = {})
  bar = options.fetch(:bar, false)
  puts bar
end

# good
def some_method(bar: false)
  puts bar
end
```

With `AllowedMethods: ['some_method']`:

```ruby
# good
def some_method(bar = false)
  puts bar
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `respond_to_missing?` |  | Method names always allowed to take an optional boolean parameter. |

## Blind spots

This cop is unsafe: changing a method signature from a positional boolean
default to a keyword argument implicitly changes call-site behaviour for
every existing positional caller. RuboCop reports it regardless (autocorrect
is simply never offered); this port matches that -- it never suppresses the
offense on safety grounds -- but, like RuboCop, does not attempt to fix it.

# Layout/SpaceAfterMethodName

Do not put a space between a method name and the opening parenthesis in a method definition.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for space between a method name and a left parenthesis in defs.

```ruby
# bad
def func (x) end
def method= (y) end

# good
def func(x) end
def method=(y) end
```

## Options

This rule has no options.

## Blind spots

None recorded.

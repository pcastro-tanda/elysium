# Naming/BinaryOperatorParameterName

When defining binary operators, name the argument other.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Makes sure that certain binary operator methods have their sole parameter
named `other`.

```ruby
# bad
def +(amount); end

# good
def +(other); end
```

## Options

This rule has no options.

## Blind spots

None recorded.

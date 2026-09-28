# Naming/AccessorMethodName

Avoid prefixing accessor method names with `get_` or `set_`.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Applies to both instance and class methods.

NOTE: Method names starting with `get_` or `set_` only register an offense
when the methods match the expected arity for getters and setters
respectively. Getters (`get_attribute`) must have no arguments to be
registered, and setters (`set_attribute(value)`) must have exactly one.

```ruby
# bad
def set_attribute(value)
end

# good
def attribute=(value)
end

# bad
def get_attribute
end

# good
def attribute
end

# accepted, incorrect arity for getter
def get_value(attr)
end

# accepted, incorrect arity for setter
def set_value
end
```

## Options

This rule has no options.

## Blind spots

None recorded.

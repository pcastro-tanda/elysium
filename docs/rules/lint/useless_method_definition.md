# Lint/UselessMethodDefinition

Checks for useless method definitions, specifically: empty constructors and methods just delegating to `super`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

```ruby
# bad
def initialize
  super
end

def method
  super
end

# good - with default arguments
def initialize(x = Object.new)
  super
end

# good
def initialize
  super
  initialize_internals
end

def method(*args)
  super(:extra_arg, *args)
end
```

## Options

This rule has no options.

## Blind spots

This cop is unsafe: an empty constructor delegating to `super` can be intentionally overriding a parent constructor, which is bad on its own, but not what this cop reports.

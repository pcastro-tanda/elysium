# Layout/SpaceAroundEqualsInParameterDefault

Checks that the equals signs in parameter default assignments have or don't have surrounding space depending on configuration.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: space (default)

# bad
def some_method(arg1=:default, arg2=nil, arg3=[])
  # do something...
end

# good
def some_method(arg1 = :default, arg2 = nil, arg3 = [])
  # do something...
end
```

```ruby
# EnforcedStyle: no_space

# bad
def some_method(arg1 = :default, arg2 = nil, arg3 = [])
  # do something...
end

# good
def some_method(arg1=:default, arg2=nil, arg3=[])
  # do something...
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `space` | `space`, `no_space` | Whether a parameter's default-value `=` requires or forbids surrounding space. |

## Blind spots

None recorded.

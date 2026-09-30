# Layout/FirstParameterIndentation

Checks the indentation of the first parameter in a method definition.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks the indentation of the first parameter in a method definition.
Parameters after the first one are checked by `Layout/ParameterAlignment`,
not by this cop.

```ruby
# bad
def some_method(
first_param,
second_param)
  123
end
```

```ruby
# EnforcedStyle: consistent (default)
# The first parameter should always be indented one step more than the
# preceding line.

# good
def some_method(
  first_param,
second_param)
  123
end
```

```ruby
# EnforcedStyle: align_parentheses
# The first parameter should always be indented one step more than the
# opening parenthesis.

# good
def some_method(
                 first_param,
second_param)
  123
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `consistent` | `consistent`, `align_parentheses` | Whether the first parameter's indentation is always relative to the start of the line where the left parenthesis is (`consistent`), or relative to the opening parenthesis's own column (`align_parentheses`). |
| IndentationWidth | `nil` |  | Number of spaces for the first parameter's indentation, overriding `Layout/IndentationWidth`'s `Width` (which itself defaults to 2). |

## Blind spots

None recorded.

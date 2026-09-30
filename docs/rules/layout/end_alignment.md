# Layout/EndAlignment

Align ends correctly.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

```ruby
# bad
variable = if true
    end

# good (keyword)
variable = if true
           end

# good (variable)
variable = if true
end

# good (start_of_line)
puts(if true
end)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyleAlignWith | `keyword` | `keyword`, `variable`, `start_of_line` | Whether `end` lines up with the matching keyword (`keyword`), with the left-hand side of an enclosing assignment (`variable`), or with the start of the line the keyword appears on (`start_of_line`). |

## Blind spots

None recorded.

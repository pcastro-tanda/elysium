# Layout/DefEndAlignment

Align ends corresponding to defs correctly.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

```ruby
# bad
private def foo
            end

# good (start_of_line)
private def foo
end

# good (def)
private def foo
        end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyleAlignWith | `start_of_line` | `start_of_line`, `def` | Whether `end` lines up with the start of the line the `def` keyword is on (`start_of_line`, so with a `private`/`public` modifier when there is one) or with the `def` keyword itself (`def`). |

## Blind spots

None recorded.

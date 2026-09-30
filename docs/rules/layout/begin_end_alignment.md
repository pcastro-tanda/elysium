# Layout/BeginEndAlignment

Align ends corresponding to begins correctly.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

`Layout/EndAlignment` aligns with keywords by default; `||= begin` tends to
align with the start of its line instead, so this cop defaults to
`EnforcedStyleAlignWith: start_of_line`.

```ruby
# bad (start_of_line)
foo ||= begin
          do_something
        end

# good (start_of_line)
foo ||= begin
  do_something
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyleAlignWith | `start_of_line` | `start_of_line`, `begin` | Whether `end` lines up with the start of the line the `begin` keyword is on (`start_of_line`) or with the `begin` keyword itself (`begin`). |

## Blind spots

None recorded.

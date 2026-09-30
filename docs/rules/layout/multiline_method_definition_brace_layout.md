# Layout/MultilineMethodDefinitionBraceLayout

Checks that the closing brace in a method definition is either on the same line as the last method parameter, or a new line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that the closing brace in a method definition is either on the same
line as the last method parameter, or a new line.

When using the `symmetrical` (default) style:

If a method definition's opening brace is on the same line as the first
parameter of the definition, then the closing brace should be on the same
line as the last parameter of the definition.

If a method definition's opening brace is on the line above the first
parameter of the definition, then the closing brace should be on the line
below the last parameter of the definition.

When using the `new_line` style, the closing brace of a multi-line method
definition must be on the line after the last parameter of the definition.

When using the `same_line` style, the closing brace of a multi-line method
definition must be on the same line as the last parameter of the definition.

```ruby
# EnforcedStyle: symmetrical (default)

# bad
def foo(a,
  b
)
end

# bad
def foo(
  a,
  b)
end

# good
def foo(a,
  b)
end

# good
def foo(
  a,
  b
)
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `symmetrical` | `symmetrical`, `new_line`, `same_line` | Whether the closing brace mirrors the opening brace's own line (`symmetrical`), always sits on the line after the last parameter (`new_line`), or always sits on the same line as the last parameter (`same_line`). |

## Blind spots

None recorded.

# Layout/MultilineArrayBraceLayout

Checks that the closing brace in an array literal is either on the same line as the last array element, or a new line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that the closing brace in an array literal is either on the same line
as the last array element, or a new line.

When using the `symmetrical` (default) style:

If an array's opening brace is on the same line as the first element of the
array, then the closing brace should be on the same line as the last element
of the array.

If an array's opening brace is on the line above the first element of the
array, then the closing brace should be on the line below the last element
of the array.

When using the `new_line` style, the closing brace of a multi-line array
literal must be on the line after the last element of the array.

When using the `same_line` style, the closing brace of a multi-line array
literal must be on the same line as the last element of the array.

```ruby
# EnforcedStyle: symmetrical (default)

# bad
[ :a,
  :b
]

# bad
[
  :a,
  :b ]

# good
[ :a,
  :b ]

# good
[
  :a,
  :b
]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `symmetrical` | `symmetrical`, `new_line`, `same_line` | Whether the closing brace mirrors the opening brace's own line (`symmetrical`), always sits on the line after the last element (`new_line`), or always sits on the same line as the last element (`same_line`). |

## Blind spots

None recorded.

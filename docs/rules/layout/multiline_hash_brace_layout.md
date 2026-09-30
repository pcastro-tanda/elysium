# Layout/MultilineHashBraceLayout

Checks that the closing brace in a hash literal is either on the same line as the last hash element, or a new line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that the closing brace in a hash literal is either on the same line
as the last hash element, or a new line.

When using the `symmetrical` (default) style:

If a hash's opening brace is on the same line as the first element of the
hash, then the closing brace should be on the same line as the last element
of the hash.

If a hash's opening brace is on the line above the first element of the
hash, then the closing brace should be on the line below the last element
of the hash.

When using the `new_line` style, the closing brace of a multi-line hash
literal must be on the line after the last element of the hash.

When using the `same_line` style, the closing brace of a multi-line hash
literal must be on the same line as the last element of the hash.

```ruby
# EnforcedStyle: symmetrical (default)

# bad
{ a: 1,
  b: 2
}

# bad
{
  a: 1,
  b: 2 }

# good
{ a: 1,
  b: 2 }

# good
{
  a: 1,
  b: 2
}
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `symmetrical` | `symmetrical`, `new_line`, `same_line` | Whether the closing brace mirrors the opening brace's own line (`symmetrical`), always sits on the line after the last element (`new_line`), or always sits on the same line as the last element (`same_line`). |

## Blind spots

None recorded.

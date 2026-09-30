# Layout/AssignmentIndentation

Checks the indentation of the first line of the right-hand-side of a multi-line assignment.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

The first line of a right-hand side that starts on its own line is indented
one `IndentationWidth` past the start of the assignment.

```ruby
# bad
value =
if foo
  'bar'
end

# good
value =
  if foo
    'bar'
  end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| IndentationWidth | `nil` |  | Number of spaces the right-hand side is indented by, overriding `Layout/IndentationWidth`'s `Width` (which itself defaults to 2). |

## Blind spots

None recorded.

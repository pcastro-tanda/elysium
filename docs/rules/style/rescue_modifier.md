# Style/RescueModifier

Avoid using `rescue` in its modifier form.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

The syntax of modifier form `rescue` can be misleading because it might lead us to believe that `rescue` handles the given exception but it actually rescues all exceptions to return the given rescue block. In this case, value returned by handle_error or SomeException.

Modifier form `rescue` would rescue all the exceptions. It would silently skip all exceptions or errors and handle the error. Example: If `NoMethodError` is raised, modifier form rescue would handle the exception.

```ruby
# bad
some_method rescue handle_error

# bad
some_method rescue SomeException

# good
begin
  some_method
rescue
  handle_error
end

# good
begin
  some_method
rescue SomeException
  handle_error
end
```

## Options

This rule has no options.

## Blind spots

`Layout/IndentationWidth`'s `Width` is read as a peer option (falling back to 2), matching upstream's `Alignment#configured_indentation_width`. `ParenthesesCorrector`'s handling of a comment above the closing paren, a chained call after it, ternary spacing, and an orphaned trailing comma is not reproduced -- only plain paren removal is ported, since no fixture exercises those cases.

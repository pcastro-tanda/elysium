# Lint/DuplicateRescueException

Checks that there are no repeated exceptions used in `rescue` expressions.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks that there are no repeated exceptions
used in `rescue` expressions.

```ruby
# bad
begin
  something
rescue FirstException
  handle_exception
rescue FirstException
  handle_other_exception
end

# good
begin
  something
rescue FirstException
  handle_exception
rescue SecondException
  handle_other_exception
end
```

## Options

This rule has no options.

## Blind spots

Duplicate detection compares each exception expression's raw source text
rather than RuboCop's true structural `Node#==`; two genuinely equal
expressions written with different incidental formatting (extra whitespace
around `::`, for instance) are treated as distinct and not flagged.

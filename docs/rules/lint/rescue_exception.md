# Lint/RescueException

Checks for `rescue` blocks targeting the `Exception` class.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for `rescue` blocks targeting the `Exception` class.

```ruby
# bad
begin
  do_something
rescue Exception
  handle_exception
end

# good
begin
  do_something
rescue ArgumentError
  handle_exception
end
```

## Options

This rule has no options.

## Blind spots

Matches only `rescue`-clause exceptions whose shape is a bare or toplevel-qualified constant read, same as upstream's `const_name` (which requires every namespace segment to itself already be a constant): a splat (`rescue *ERRORS`), a method call, or a namespace held in a local variable (`rescue adapter::ParseError`) can never match, matching upstream's own "does not crash" specs for those shapes.

# Lint/EmptyEnsure

Checks for empty ensure block.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for empty `ensure` blocks.

```ruby
# bad
def some_method
  do_something
ensure
end

# bad
begin
  do_something
ensure
end

# good
def some_method
  do_something
ensure
  do_something_else
end

# good
begin
  do_something
ensure
  do_something_else
end
```

## Options

This rule has no options.

## Blind spots

None recorded.

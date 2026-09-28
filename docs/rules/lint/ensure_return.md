# Lint/EnsureReturn

Checks for `return` from an `ensure` block.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for `return` from an `ensure` block. `return` from an ensure block is a dangerous code
smell as it will take precedence over any exception being raised, and the exception will be
silently thrown away as if it were rescued.

If you want to rescue some (or all) exceptions, best to do it explicitly.

```ruby
# bad
def foo
  do_something
ensure
  cleanup
  return self
end

# good
def foo
  do_something
  self
ensure
  cleanup
end

# good
def foo
  begin
    do_something
  rescue SomeException
    # Let's ignore this exception
  end
  self
ensure
  cleanup
end
```

## Options

This rule has no options.

## Blind spots

Matches upstream's own blind spot: `each_node`/`each_descendant` do not respect scope boundaries,
so a `return` nested inside a `def`, block, or lambda written inside the `ensure` body is still
flagged even though it returns from that inner scope, not from the method the `ensure` belongs to.

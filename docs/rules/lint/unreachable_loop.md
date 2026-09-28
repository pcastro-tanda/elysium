# Lint/UnreachableLoop

Checks for loops that will have at most one iteration.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for loops that will have at most one iteration.

A loop that can never reach the second iteration is a possible error in the
code. In rare cases where only one iteration (or at most one iteration) is
intended behavior, the code should be refactored to use `if` conditionals.

NOTE: Block methods that are used with `Enumerable`s are considered to be
loops.

`AllowedPatterns` can be used to match against the block receiver in order
to allow code that would otherwise be registered as an offense (eg. `times`
used not in an `Enumerable` context).

```ruby
# bad
while node
  do_something(node)
  node = node.parent
  break
end

# good
while node
  do_something(node)
  node = node.parent
end

# bad
def verify_list(head)
  item = head
  begin
    if verify(item)
      return true
    else
      return false
    end
  end while(item)
end

# good
def verify_list(head)
  item = head
  begin
    if verify(item)
      item = item.next
    else
      return false
    end
  end while(item)

  true
end

# bad
def find_something(items)
  items.each do |item|
    if something?(item)
      return item
    else
      raise NotFoundError
    end
  end
end

# good
def find_something(items)
  items.each do |item|
    if something?(item)
      return item
    end
  end
  raise NotFoundError
end

# bad
2.times { raise ArgumentError }
```

With `AllowedPatterns: ['(exactly|at_least|at_most)\(\d+\)\.times']` (the default):

```ruby
# good
exactly(2).times { raise StandardError }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedPatterns | `(exactly|at_least|at_most)\(\d+\)\.times` |  | Method call source patterns (matched against the loop-like block's owning call, excluding the block itself) that are never flagged. |

## Blind spots

`conditional_continue_keyword?`'s `each_descendant(:or).to_a.last` takes the last `or` node in traversal (pre-)order, which for a chain of more than one `||` is not necessarily the last one written in source (every fixture has at most one `||`, so this never diverges in practice).

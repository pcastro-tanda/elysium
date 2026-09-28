# Lint/ReturnInVoidContext

Checks for the use of a return with a value in a context where the value will be ignored.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for the use of a return with a value in a context
where the value will be ignored. (initialize and setter methods)

```ruby
# bad
def initialize
  foo
  return :qux if bar?
  baz
end

def foo=(bar)
  return 42
end

# good
def initialize
  foo
  return if bar?
  baz
end

def foo=(bar)
  return
end
```

## Options

This rule has no options.

## Blind spots

`assignment_method?` does not check `def_type?`, so a singleton setter
(`def self.foo=`) is void context too, exactly like an instance setter --
mirrored here rather than treated as a bug. `each_ancestor(:any_block)` is
unbounded: a scope-changing block/lambda *outside* the enclosing `def`
(which cannot normally contain a `return` reaching back into it) is, like
upstream, never distinguished from one directly wrapping the `return`.

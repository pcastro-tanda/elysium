# Style/For

Checks use of for or each in multiline loops.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Looks for uses of the `for` keyword or `each` method. The preferred alternative is set in the `EnforcedStyle` configuration parameter. An `each` call with a block on a single line is always allowed.

NOTE: `each` is preferred in idiomatic Ruby because `for` leaks its loop variable into the surrounding scope.

```ruby
# EnforcedStyle: each (default)
# bad
def foo
  for n in [1, 2, 3] do
    puts n
  end
end

# good
def foo
  [1, 2, 3].each do |n|
    puts n
  end
end
```

```ruby
# EnforcedStyle: for
# bad
def foo
  [1, 2, 3].each do |n|
    puts n
  end
end

# good
def foo
  for n in [1, 2, 3] do
    puts n
  end
end
```

This cop's autocorrection is unsafe because the scope of variables is different between `each` and `for`.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `each` | `each`, `for` | Whether to prefer `for` or `each` for multiline loops. |

## Blind spots

None recorded.

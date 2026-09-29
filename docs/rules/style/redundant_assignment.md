# Style/RedundantAssignment

Checks for redundant assignment before returning.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for redundant assignment before returning.

```ruby
# bad
def test
  x = foo
  x
end

# bad
def test
  if x
    z = foo
    z
  elsif y
    z = bar
    z
  end
end

# good
def test
  foo
end

# good
def test
  if x
    foo
  elsif y
    bar
  end
end
```

When there are comments between the assignment and reference, the cop will
report an offense but it will not autocorrect.

## Options

This rule has no options.

## Blind spots

None recorded.

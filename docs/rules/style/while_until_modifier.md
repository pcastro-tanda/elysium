# Style/WhileUntilModifier

Favor modifier while/until usage when you have a single-line body.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for `while` and `until` statements that would fit on one line if
written as a modifier `while`/`until`. The maximum line length is configured
in the `Layout/LineLength` cop.

```ruby
# bad
while x < 10
  x += 1
end

# good
x += 1 while x < 10

# good
while x < 10
  y += 1 if x.odd?
end

# bad
until x > 10
  x += 1
end

# good
x += 1 until x > 10

# good
until x > 10
  y += 1 unless x.even?
end

# bad
x += 100 while x < 500 # a long comment that makes code too long if it were a single line

# good
while x < 500 # a long comment that makes code too long if it were a single line
  x += 100
end
```

## Options

This rule has no options.

## Blind spots

`Node#parent` (used by `parenthesize?`) is reconstructed from a whole-file
traversal recording every node shape known to need parentheses (assignment
targets, `&&`/`||`, array elements, hash values, call receivers/arguments)
rather than true parent pointers; a `while`/`until` in some other position
(e.g. a block argument) is treated as never needing parentheses, which only
risks false negatives.

`if_body_source`'s omitted-hash-value reconstruction only special-cases a
call whose last argument is a hash/keyword-hash with a value-omitted last
pair (`obj.foo bar:`); other RuboCop-recognized shapes for that rewrite fall
back to the body's raw source, which is usually byte-identical anyway.

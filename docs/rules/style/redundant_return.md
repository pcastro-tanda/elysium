# Style/RedundantReturn

Don't use return where it's not required.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

A `return` at the end of a method, or as the last executed expression of an
`if`/`case`/`begin`/`rescue` branch within one, is unnecessary: the value
of the last expression is already the method's return value.

```ruby
# bad
def test
  return something
end

# bad
def test
  one
  two
  three
  return something
end

# bad
def test
  return something if something_else
end

# good
def test
  something if something_else
end

# good
def test
  if x
  elsif y
  else
  end
end
```

The same check applies to `define_method`/`define_singleton_method`/`lambda`
blocks and `->` literals.

@example AllowMultipleReturnValues: false (default)
```ruby
# bad
def test
  return x, y
end
```

@example AllowMultipleReturnValues: true
```ruby
# good
def test
  return x, y
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowMultipleReturnValues | false |  | When `true`, allows code like `return x, y`. |

## Blind spots

Matches RuboCop's own scope exactly: `define_method`/`define_singleton_method`/
`lambda` are matched by bare method name regardless of receiver, so
`Foo.lambda { return x }` is flagged too (an upstream imprecision, preserved
here for parity, not a gap specific to this port). The splat-argument
autocorrect only strips a leading `*` off the *first* return value (mirroring
RuboCop's own `first_argument`-based correction), so `return *a, b` where the
splat is not literally the first value is left with its `*` (also inherited
from upstream, not a new limitation).

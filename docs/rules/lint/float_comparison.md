# Lint/FloatComparison

Checks for the presence of precise comparison of floating point numbers.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Floating point values are inherently inaccurate, and comparing them for exact equality
is almost never the desired semantics. Comparison via the `==`/`!=` operators checks
floating-point value representation to be exactly the same, which is very unlikely
if you perform any arithmetic operations involving precision loss.

```ruby
# bad
x == 0.1
x != 0.1

# bad
case value
when 1.0
  foo
when 2.0
  bar
end

# good - using BigDecimal
x.to_d == 0.1.to_d

# good - comparing against zero
x == 0.0
x != 0.0

# good
(x - 0.1).abs < Float::EPSILON

# good
tolerance = 0.0001
(x - 0.1).abs < tolerance

# good - comparing against nil
Float(x, exception: false) == nil

# good - using epsilon comparison in case expression
case
when (value - 1.0).abs < Float::EPSILON
  foo
when (value - 2.0).abs < Float::EPSILON
  bar
end
```

## Options

This rule has no options.

## Blind spots

`float_send?`'s instance-method branch (upstream's `node.receiver&.float_type?`) only
recognizes a receiver that is *literally* a float literal, not one that merely produces a
float, so e.g. `x.to_f.abs == 0.1` is not flagged via that path -- an upstream limitation,
not one introduced by this port. `numeric_returning_method?`'s `angle`/`arg`/`phase` and
`ceil`/`floor`/`round`/`truncate` branches parse the receiver's/argument's own source text
as a number, exactly as upstream's `Float(...)`/`Integer(...)` calls on `node.source`.

# Performance/BigDecimalWithNumericArgument

Convert numeric literal to string and pass it to `BigDecimal`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where a float argument to `BigDecimal` should be converted to a string. Initializing from String is faster than from Float for BigDecimal.

Also identifies places where an integer string argument to BigDecimal should be converted to an integer. Initializing from Integer is faster than from String for BigDecimal.

```ruby
# bad
BigDecimal(1.2, 3, exception: true)
4.5.to_d(6, exception: true)

# good
BigDecimal('1.2', 3, exception: true)
BigDecimal('4.5', 6, exception: true)

# bad
BigDecimal('1', 2)
BigDecimal('4', 6)

# good
BigDecimal(1, 2)
4.to_d(6)
```

## Options

This rule has no options.

## Blind spots

None recorded.

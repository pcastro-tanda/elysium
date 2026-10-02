# Lint/NumericOperationWithConstantResult

Checks for numeric operations with constant results.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Certain numeric operations have a constant result, usually 0 or 1.
Multiplying a number by 0 will always return 0.
Dividing a number by itself or raising it to the power of 0 will always return 1.
As such, they can be replaced with that result.
These are probably leftover from debugging, or are mistakes.
Other numeric operations that are similarly leftover from debugging or mistakes
are handled by `Lint/UselessNumericOperation`.

NOTE: This cop doesn't detect offenses for the `-` and `%` operator because it
can't determine the type of `x`. If `x` is an `Array` or `String`, it doesn't perform
a numeric operation.

@safety
  This cop is unsafe because the autocorrection drops the operands, which
  discards any side effects of evaluating them and can change behavior when
  the result is not actually constant. For example, `x / x` raises
  `ZeroDivisionError` when `x` is `0`, and returns `Float::NAN` (not `1`)
  when `x` is `0.0`; replacing it with `1` silences that.

```ruby
# bad
x * 0

# good
0

# bad
x *= 0

# good
x = 0

# bad
x / x
x ** 0

# good
1

# bad
x /= x
x **= 0

# good
x = 1
```

## Options

This rule has no options.

## Blind spots

None recorded.

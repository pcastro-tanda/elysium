# Performance/Sum

Use `sum` instead of a custom array summation.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies places where custom code finding the sum of elements in some Enumerable object can be replaced by `Enumerable#sum` method.

Autocorrections are unproblematic wherever an initial value is provided explicitly. When no initial value is provided, `Enumerable#reduce` picks the first enumerated value as initial value whereas `Enumerable#sum` sets an initial value of `0`, which can lead to a `TypeError`; those autocorrections are unsafe.

```ruby
# bad
[1, 2, 3].inject(:+)
[1, 2, 3].inject(&:+)
[1, 2, 3].reduce { |acc, elem| acc + elem }
[1, 2, 3].reduce(10, :+)
[1, 2, 3].map { |elem| elem ** 2 }.sum
[1, 2, 3].collect(&:count).sum(10)

# good
[1, 2, 3].sum
[1, 2, 3].sum(10)
[1, 2, 3].sum { |elem| elem ** 2 }
[1, 2, 3].sum(10, &:count)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| SafeAutoCorrect | false |  | Do not autocorrect when no initial value is given. |
| OnlySumOrWithInitialValue | false |  | Only flag `sum` candidates that carry an initial value. |

## Blind spots

None recorded.

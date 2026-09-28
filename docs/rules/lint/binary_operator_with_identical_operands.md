# Lint/BinaryOperatorWithIdenticalOperands

Checks for places where binary operator has identical operands.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for places where binary operator has identical operands.

It covers comparison operators: `==`, `===`, `=~`, `>`, `>=`, `<`, `<=`;
bitwise operators: `|`, `^`, `&`;
boolean operators: `&&`, `||`
and "spaceship" operator - `<=>`.

Simple arithmetic operations are allowed by this cop: `+`, `*`, `**`, `<<` and `>>`.
Although these can be rewritten in a different way, it should not be necessary to
do so. Operations such as `-` or `/` where the result will always be the same
(`x - x` will always be 0; `x / x` will always be 1) are offenses, but these
are covered by `Lint/NumericOperationWithConstantResult` instead.

```ruby
# bad
x.top >= x.top

if a.x != 0 && a.x != 0
  do_something
end

def child?
  left_child || left_child
end

# good
x + x
1 << 1
```

## Options

This rule has no options.

## Blind spots

This cop is unsafe as it does not consider side effects when calling methods and thus can
generate false positives (e.g. `wr.take_char == '\0' && wr.take_char == '\0'`); elysium does not
model side effects either, so this is inherited rather than newly introduced. Operand equality is
approximated by exact source-text comparison rather than RuboCop's true structural `Node#==`: a
genuinely equal expression written with different incidental formatting is treated as unequal.

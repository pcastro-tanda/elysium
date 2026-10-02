# Lint/UselessNumericOperation

Checks for useless numeric operations.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Certain numeric operations have no impact, being: Adding or subtracting 0,
multiplying or dividing by 1 or raising to the power of 1. These are
probably leftover from debugging, or are mistakes.

```ruby
# bad
x + 0
x - 0
x * 1
x / 1
x ** 1

# good
x

# bad
x += 0
x -= 0
x *= 1
x /= 1
x **= 1

# good
x = x
```

## Options

This rule has no options.

## Blind spots

A qualified constant receiver/target (`Foo::BAR + 0`, `Foo::BAR *= 1`) is not matched: Prism splits whitequark's single `const`/`casgn` node type (which recurses to cover both a bare and a qualified constant) into `ConstantReadNode`/`ConstantOperatorWriteNode` (bare) versus `ConstantPathNode`/`ConstantPathOperatorWriteNode` (qualified); this rule, matching only the bare kinds per the node-pattern's `const`/`casgn` alternative, never sees the qualified ones.

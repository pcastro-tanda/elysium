# Style/RedundantFilterChain

Identifies usages of `any?`, `empty?`, `none?` or `one?` predicate methods chained to `select`/`filter`/`find_all` and change them to use predicate method instead.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies usages of `any?`, `empty?` or `none?` predicate methods
chained to `select`/`filter`/`find_all` and change them to use predicate method instead.

```ruby
# bad
arr.select { |x| x > 1 }.any?

# good
arr.any? { |x| x > 1 }

# bad
arr.select { |x| x > 1 }.empty?
arr.select { |x| x > 1 }.none?

# good
arr.none? { |x| x > 1 }

# good
relation.select(:name).any?
arr.select { |x| x > 1 }.any?(&:odd?)
```

With `AllCops: ActiveSupportExtensionsEnabled: true`, `many?` and
`present?` are treated the same way (`many?`/`any?` respectively).

## Options

This rule has no options.

## Blind spots

`array.select.any?` evaluates every element through `select`'s own
enumeration, while `array.any?` short-circuits on the first match; the
autocorrect is marked unsafe (matching upstream's `SafeAutoCorrect: false`)
but this port does not otherwise special-case side-effecting blocks.

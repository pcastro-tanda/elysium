# Style/RedundantFetchBlock

Use `fetch(key, value)` instead of `fetch(key) { value }` when value has Numeric, Rational, Complex, Symbol or String type, `false`, `true`, `nil` or is a constant.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies places where `fetch(key) { value }` can be replaced by
`fetch(key, value)`.

In such cases `fetch(key, value)` method is faster than `fetch(key) { value }`.

NOTE: The block string `'value'` in `hash.fetch(:key) { 'value' }` is
detected when frozen string literal magic comment is enabled (i.e. `#
frozen_string_literal: true`), but not when disabled.

@safety
This cop is unsafe because it cannot be guaranteed that the receiver does
not have a different implementation of `fetch`.

```ruby
# bad
hash.fetch(:key) { 5 }
hash.fetch(:key) { true }
hash.fetch(:key) { nil }
array.fetch(5) { :value }
ENV.fetch(:key) { 'value' }

# good
hash.fetch(:key, 5)
hash.fetch(:key, true)
hash.fetch(:key, nil)
array.fetch(5, :value)
ENV.fetch(:key, 'value')
```

With `SafeForConstants: true`:

```ruby
# bad
ENV.fetch(:key) { VALUE }

# good
ENV.fetch(:key, VALUE)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| SafeForConstants | false |  | Whether a constant default value block is also flagged (unsafe: the constant may be reassigned before the block would have run). |

## Blind spots

None recorded.

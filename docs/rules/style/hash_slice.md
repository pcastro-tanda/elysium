# Style/HashSlice

Checks for usages of `Hash#reject`, `Hash#select`, and `Hash#filter` methods that can be replaced with `Hash#slice` method.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

This cop should only be enabled on Ruby version 2.5 or higher (`Hash#slice`
was added in Ruby 2.5).

For safe detection, it is limited to commonly used string and symbol
comparisons when using `==` or `!=`.

This cop doesn't check for `Hash#delete_if` and `Hash#keep_if` because they
modify the receiver.

This cop is unsafe because it cannot be guaranteed that the receiver is a
`Hash` or responds to the replacement method.

Additionally, the replacement may change the order of the resulting hash:
`Hash#slice` returns entries in the order the keys are given, whereas
`select`, `filter`, and `reject` preserve the entry order of the receiver.

```ruby
# bad
{foo: 1, bar: 2, baz: 3}.select {|k, v| k == :bar }
{foo: 1, bar: 2, baz: 3}.reject {|k, v| k != :bar }
{foo: 1, bar: 2, baz: 3}.filter {|k, v| k == :bar }
{foo: 1, bar: 2, baz: 3}.select {|k, v| k.eql?(:bar) }

# bad
{foo: 1, bar: 2, baz: 3}.select {|k, v| %i[bar].include?(k) }
{foo: 1, bar: 2, baz: 3}.reject {|k, v| !%i[bar].include?(k) }
{foo: 1, bar: 2, baz: 3}.filter {|k, v| %i[bar].include?(k) }

# good
{foo: 1, bar: 2, baz: 3}.slice(:bar)
```

## Options

This rule has no options.

## Blind spots

None recorded.

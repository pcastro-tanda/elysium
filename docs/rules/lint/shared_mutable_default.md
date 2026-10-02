# Lint/SharedMutableDefault

Checks for mutable literals used as default arguments during Hash initialization.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Creating a `Hash` in such a way will share the default value
across all keys, causing unexpected behavior when modifying it.

For example, when the `Hash` was created with an `Array` as the argument,
calling `hash[:foo] << 'bar'` will also change the value of all
other keys that have not been explicitly assigned to.

```ruby
# bad
Hash.new([])
Hash.new({})
Hash.new(Array.new)
Hash.new(Hash.new)

# okay -- In rare cases that intentionally have this behavior,
#   without disabling the cop, you can set the default explicitly.
h = Hash.new
h.default = []
h[:a] << 1
h[:b] << 2
h # => {:a => [1, 2], :b => [1, 2]}

# okay -- beware this will discard mutations and only remember assignments
Hash.new { Array.new }
Hash.new { Hash.new }
Hash.new { {} }
Hash.new { [] }

# good - frozen solution will raise an error when mutation attempted
Hash.new([].freeze)
Hash.new({}.freeze)

# good - using a proc will create a new object for each key
h = Hash.new
h.default_proc = ->(h, k) { [] }
h.default_proc = ->(h, k) { {} }

# good - using a block will create a new object for each key
Hash.new { |h, k| h[k] = [] }
Hash.new { |h, k| h[k] = {} }
```

## Options

This rule has no options.

## Blind spots

None recorded.

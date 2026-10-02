# Lint/HashNewWithKeywordArgumentsAsDefault

Checks for the deprecated use of keyword arguments for hash default in `Hash.new`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for the deprecated use of keyword arguments as a default in `Hash.new`.

This usage raises a warning in Ruby 3.3 and results in an error in Ruby 3.4.
In Ruby 3.4, keyword arguments will instead be used to change the behavior of
a hash. For example, the capacity option can be passed to create a hash with
a certain size if you know it in advance, for better performance.

```ruby
# bad
Hash.new(key: :value)

# good
Hash.new({key: :value})
```

## Options

This rule has no options.

## Blind spots

None recorded.

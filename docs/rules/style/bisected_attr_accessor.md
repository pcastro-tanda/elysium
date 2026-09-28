# Style/BisectedAttrAccessor

Checks for places where `attr_reader` and `attr_writer` for the same method can be combined into single `attr_accessor`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
class Foo
  attr_reader :bar
  attr_writer :bar
end

# good
class Foo
  attr_accessor :bar
end
```

## Options

This rule has no options.

## Blind spots

None recorded.

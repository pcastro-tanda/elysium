# Style/TrailingBodyOnClass

Class body goes below class statement.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
class Foo; def foo; end
end

# good
class Foo
  def foo; end
end
```

## Options

This rule has no options.

## Blind spots

`Layout/IndentationWidth`'s `Width` is read as a peer option (falling back to 2), matching upstream's `Alignment#configured_indentation_width`.

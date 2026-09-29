# Style/TrailingBodyOnModule

Checks for trailing code after the module definition.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
module Foo extend self
end

# good
module Foo
  extend self
end
```

## Options

This rule has no options.

## Blind spots

`Layout/IndentationWidth`'s `Width` is read as a peer option, matching
upstream's own `Alignment#configured_indentation_width`.

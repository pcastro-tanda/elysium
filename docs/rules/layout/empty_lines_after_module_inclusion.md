# Layout/EmptyLinesAfterModuleInclusion

Keeps track of empty lines after module inclusion methods.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for an empty line after a module inclusion method (`extend`,
`include` and `prepend`), or a group of them.

```ruby
# bad
class Foo
  include Bar
  attr_reader :baz
end

# good
class Foo
  include Bar

  attr_reader :baz
end

# also good - multiple module inclusions grouped together
class Foo
  extend Bar
  include Baz
  prepend Qux
end
```

## Options

This rule has no options.

## Blind spots

None recorded.

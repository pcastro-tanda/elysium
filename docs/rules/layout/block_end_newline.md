# Layout/BlockEndNewline

Put end statement of multiline block on its own line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
blah do |i|
  foo(i) end

# good
blah do |i|
  foo(i)
end

# bad
blah { |i|
  foo(i) }

# good
blah { |i|
  foo(i)
}
```

## Options

This rule has no options.

## Blind spots

None recorded.

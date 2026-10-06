# Performance/StringIdentifierArgument

Use symbol identifier argument instead of string identifier argument.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where string identifier argument can be replaced by symbol identifier argument. It prevents the redundancy of the internal string-to-symbol conversion.

```ruby
# bad
send('do_something')
attr_accessor 'do_something'
instance_variable_get('@ivar')

# good
send(:do_something)
attr_accessor :do_something
instance_variable_get(:@ivar)
```

## Options

This rule has no options.

## Blind spots

None recorded.

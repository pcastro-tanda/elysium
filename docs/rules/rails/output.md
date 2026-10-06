# Rails/Output

Checks for calls to puts, print, etc.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for the use of output calls like puts and print

This cop's autocorrection is unsafe because depending on the Rails log level configuration, changing from `puts` to `Rails.logger.debug` could result in no output being shown.

```ruby
# bad
puts 'A debug message'
pp 'A debug message'
print 'A debug message'

# good
Rails.logger.debug 'A debug message'
```

## Options

This rule has no options.

## Blind spots

None recorded.

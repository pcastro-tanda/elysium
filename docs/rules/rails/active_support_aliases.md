# Rails/ActiveSupportAliases

Avoid ActiveSupport aliases of standard ruby methods: `String#starts_with?`, `String#ends_with?`, `Array#append`, `Array#prepend`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that ActiveSupport aliases to core ruby methods are not used.

```ruby
# good
'some_string'.start_with?('prefix')
'some_string'.end_with?('suffix')
[1, 2, 'a'] << 'b'
[1, 2, 'a'].unshift('b')

# bad
'some_string'.starts_with?('prefix')
'some_string'.ends_with?('suffix')
[1, 2, 'a'].append('b')
[1, 2, 'a'].prepend('b')
```

## Options

This rule has no options.

## Blind spots

None recorded.

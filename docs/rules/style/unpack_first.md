# Style/UnpackFirst

Checks for accessing the first element of `String#unpack` instead of using `unpack1`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for accessing the first element of `String#unpack`
which can be replaced with the shorter method `unpack1`.

# Examples

```ruby
# bad
'foo'.unpack('h*').first
'foo'.unpack('h*')[0]
'foo'.unpack('h*').slice(0)
'foo'.unpack('h*').at(0)

# good
'foo'.unpack1('h*')
```

## Options

This rule has no options.

## Blind spots

None recorded.

# Lint/RedundantStringCoercion

Checks for `Object#to_s` usage in string interpolation.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for string conversion in string interpolation, `print`, `puts`, and
`warn` arguments, which is redundant.

```ruby
# bad
"result is #{something.to_s}"
print something.to_s
puts something.to_s
warn something.to_s

# good
"result is #{something}"
print something
puts something
warn something
```

## Options

This rule has no options.

## Blind spots

None recorded.

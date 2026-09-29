# Lint/ImplicitStringConcatenation

Checks for adjacent string literals on the same line, which could better be represented as a single string literal.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for implicit string concatenation of string literals
which are on the same line.

```ruby
# bad
array = ['Item 1' 'Item 2']

# good
array = ['Item 1Item 2']
array = ['Item 1' + 'Item 2']
array = [
  'Item 1' \
  'Item 2'
]
```

## Options

This rule has no options.

## Blind spots

None recorded.

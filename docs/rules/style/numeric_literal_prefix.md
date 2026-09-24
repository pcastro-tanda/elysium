# Style/NumericLiteralPrefix

Use smallcase prefixes for numeric literals.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

```ruby
# EnforcedOctalStyle: zero_with_o (default)

# bad - missing octal prefix
num = 01234

# bad - uppercase prefix
num = 0O1234
num = 0X12AB
num = 0B10101

# bad - redundant decimal prefix
num = 0D1234
num = 0d1234

# good
num = 0o1234
num = 0x12AB
num = 0b10101
num = 1234
```

```ruby
# EnforcedOctalStyle: zero_only

# bad
num = 0o1234
num = 0O1234

# good
num = 01234
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedOctalStyle | `zero_with_o` | `zero_with_o`, `zero_only` | Whether octal literals must use the `0o` prefix or a bare `0`. |

## Blind spots

`HEX_REGEX`/`BINARY_REGEX` are ported byte-for-byte from RuboCop's own
case-sensitive regexes: a literal like `0X1ac` (uppercase `X` prefix but a
lowercase hex digit) or `0xABC` (lowercase `x` prefix, uppercase digits, thus
already 'correct' and left alone) is not touched by either RuboCop or this
port, since the whole digit run's case has to agree with the regex before it
is considered a match at all; this is upstream's own limitation, not a gap
introduced here. Because Prism never fuses a leading unary `-`/`+` sign onto
an `IntegerNode` (unlike RuboCop's own parser), a signed literal such as
`-0X1AC` is reported/fixed at the narrower span of the literal alone
(excluding the sign); RuboCop's fix for that exact shape is actually a
no-op bug (its corrector operates on the sign-inclusive node source, which
no longer matches the anchored `^0X` prefix regex), so this port's behavior
is strictly more correct there rather than a regression.

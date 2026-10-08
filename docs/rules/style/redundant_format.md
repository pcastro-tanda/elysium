# Style/RedundantFormat

Checks for usages of `Kernel#format` or `Kernel#sprintf` with only a single argument.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Calling `format` with only a single string or constant argument is redundant,
as it can be replaced by the string or constant itself.

Also looks for `format` calls where the arguments are literals that can be
inlined into a string easily. This applies to the `%s`, `%d`, `%i`, `%u`, and
`%f` format specifiers.

```ruby
# bad
format('the quick brown fox jumps over the lazy dog.')
sprintf('the quick brown fox jumps over the lazy dog.')

# good
'the quick brown fox jumps over the lazy dog.'

# bad
format(MESSAGE)
sprintf(MESSAGE)

# good
MESSAGE

# bad
format('%s %s', 'foo', 'bar')
sprintf('%s %s', 'foo', 'bar')

# good
'foo bar'
```

## Options

This rule has no options.

## Blind spots

Format-sequence parsing only recognizes the canonical `<name>`/`{name}`
ordering (right after flags, before width/precision), drops the
`TEMPLATE_NAME` lookbehind that disambiguates a literal `%#{...}` text run
from a `{name}` template, and `Rational`/`Complex` literal values only cover
the exact `int / Nr` and `int + Ni` shapes upstream's own node matchers
recognize (not arbitrary nested arithmetic).

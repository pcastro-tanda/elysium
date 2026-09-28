# Style/LineEndConcatenation

Use \ instead of + or << to concatenate two string literals at line end.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for string literal concatenation at the end of a line.

```ruby
# bad
some_str = 'ala' +
           'bala'

some_str = 'ala' <<
           'bala'

# good
some_str = 'ala' \
           'bala'
```

## Options

This rule has no options.

## Blind spots

Autocorrection is unsafe because the receiver isn't guaranteed to be a
string: replacing `<<` with `\` when the receiver is e.g. an array (`array
<< 'foo' <<\n'bar'`) would produce a syntax error.

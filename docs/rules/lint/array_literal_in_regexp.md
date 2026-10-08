# Lint/ArrayLiteralInRegexp

Checks for an array literal interpolated inside a regexp.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for an array literal interpolated inside a regexp.

When interpolating an array literal, it is converted to a string. This means that when inside a regexp, it acts as a character class but with additional quotes, spaces and commas that are likely not intended. For example, `/#{%w[a b c]}/` parses as `/["a", "b", "c"]/` (or `/["a, bc]/` without repeated characters).

The cop can autocorrect to a character class (if all items in the array are a single character) or alternation (if the array contains longer items).

NOTE: This only considers interpolated arrays that contain only strings, symbols, integers, and floats. Any other type is not easily convertible to a character class or regexp alternation.

```ruby
# bad
/#{%w[a b c]}/

# good
/[abc]/

# bad
/#{%w[foo bar baz]}/

# good
/(?:foo|bar|baz)/

# bad - construct a regexp rather than interpolate an array of identifiers
/#{[foo, bar]}/
```

## Options

This rule has no options.

## Blind spots

Control characters outside the whitespace/`#`/metacharacter set that Ruby's `Regexp.escape` treats specially (e.g. `\x01`) are copied verbatim instead of escaped, since no fixture exercises them.

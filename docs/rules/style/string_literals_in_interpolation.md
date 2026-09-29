# Style/StringLiteralsInInterpolation

Checks if uses of quotes inside expressions in interpolated strings match the configured preference.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that quotes inside string, symbol, and regexp interpolations match
the configured preference.

```ruby
# EnforcedStyle: single_quotes (default)

# bad
string = "Tests #{success ? "PASS" : "FAIL"}"
symbol = :"Tests #{success ? "PASS" : "FAIL"}"
regexp = /Tests #{success ? "PASS" : "FAIL"}/

# good
string = "Tests #{success ? 'PASS' : 'FAIL'}"
symbol = :"Tests #{success ? 'PASS' : 'FAIL'}"
regexp = /Tests #{success ? 'PASS' : 'FAIL'}/
```

```ruby
# EnforcedStyle: double_quotes

# bad
string = "Tests #{success ? 'PASS' : 'FAIL'}"
symbol = :"Tests #{success ? 'PASS' : 'FAIL'}"
regexp = /Tests #{success ? 'PASS' : 'FAIL'}/

# good
string = "Tests #{success ? "PASS" : "FAIL"}"
symbol = :"Tests #{success ? "PASS" : "FAIL"}"
regexp = /Tests #{success ? "PASS" : "FAIL"}/
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `single_quotes` | `single_quotes`, `double_quotes` | Preferred string literal quote style for interpolated expressions. |

## Blind spots

Ports whitequark's `inside_interpolation?` with nesting counters rather than
a true ancestor walk (matching `Style/StringLiterals`'s port), so it does
not distinguish further by container kind beyond string/symbol/regexp vs.
everything else, which matches RuboCop's own behaviour.

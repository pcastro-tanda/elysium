# Style/ExponentialNotation

Enforces consistency when using exponential notation for numbers in the code.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Enforces consistency when using exponential notation
for numbers in the code (eg `1.2e4`). Different styles are supported:

* `scientific` which enforces a mantissa between 1 (inclusive) and 10 (exclusive).
* `engineering` which enforces the exponent to be a multiple of 3 and the mantissa
  to be between 0.1 (inclusive) and 1000 (exclusive).
* `integral` which enforces the mantissa to always be a whole number without
  trailing zeroes.

```ruby
# EnforcedStyle: scientific (default)
# Enforces a mantissa between 1 (inclusive) and 10 (exclusive).

# bad
10e6
0.3e4
11.7e5
3.14e0

# good
1e7
3e3
1.17e6
3.14
```

```ruby
# EnforcedStyle: engineering
# Enforces using multiple of 3 exponents,
# mantissa should be between 0.1 (inclusive) and 1000 (exclusive)

# bad
3.2e7
0.1e5
12e5
1232e6

# good
32e6
10e3
1.2e6
1.232e9
```

```ruby
# EnforcedStyle: integral
# Enforces the mantissa to have no decimal part and no
# trailing zeroes.

# bad
3.2e7
0.1e5
120e4

# good
32e6
1e4
12e5
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `scientific` | `scientific`, `engineering`, `integral` | Which exponential-notation mantissa/exponent convention to enforce. |

## Blind spots

An exponent letter spelled as a bare uppercase `E` (with no lowercase `e` elsewhere in the
literal's source, e.g. `1E10`) is never flagged, matching upstream's own
`node.source['e']`-guarded (lowercase-only) `offense?` check -- an upstream blind spot, not one
introduced by this port. A sign separated from the literal by whitespace parses as a `CallNode`
wrapping an unsigned `FloatNode`, which -- like upstream's `on_float` -- this cop never visits.

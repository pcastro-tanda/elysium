# Lint/FloatOutOfRange

Catches floating-point literals too large or small for Ruby to represent.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Identifies `Float` literals which are, like, really really really
really really really really big. Too big. No-one needs Floats
that big. If you need a float that big, something is wrong with you.

```ruby
# bad
float = 3.0e400

# good
float = 42.9
```

## Options

This rule has no options.

## Blind spots

The literal's source text is re-parsed with Rust's `f64::from_str` after
stripping `_` digit separators, rather than reusing a pre-parsed value from
the parser; this matches Ruby's own `Float()` semantics (round-to-nearest,
overflow to infinity, underflow to zero) for every literal Ruby's lexer
accepts, so no discrepancy is expected.

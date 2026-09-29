# Style/ZeroLengthPredicate

Use #empty? when testing for objects of length 0.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for numeric comparisons that can be replaced by a predicate method,
such as `receiver.length == 0`, `receiver.length > 0`, and
`receiver.length != 0`, `receiver.length < 1` and `receiver.size == 0` that
can be replaced by `receiver.empty?` and `!receiver.empty?`.

`File`, `Tempfile`, `StringIO`, and `File::Stat` do not have `empty?` so
this cop allows `size == 0` and `size.zero?` for them. When a `File::Stat`
object is stored in a variable, the cop cannot detect the type and may still
register a false positive.

@safety
  This cop is unsafe because it cannot be guaranteed that the receiver has
  an `empty?` method that is defined in terms of `length`. If there is a
  non-standard class that redefines `length` or `empty?`, the cop may
  register a false positive.

## Options

This rule has no options.

## Blind spots

`non_polymorphic_collection?` only recognizes the `File.stat(...)`/
`File.new(...)`/`Tempfile.new(...)`/`StringIO.new(...)`/`File::Stat.new(...)`
call shapes textually; a `File::Stat` (etc.) instance reached through a
variable or another method call is not recognized and may false-positive,
matching upstream's own documented limitation.

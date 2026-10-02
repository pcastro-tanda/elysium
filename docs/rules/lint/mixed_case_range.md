# Lint/MixedCaseRange

Checks for mixed-case character ranges since they include likely unintended characters.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Offenses are registered for regexp character classes like `/[A-z]/`
as well as range objects like `('A'..'z')`.

NOTE: `Range` objects cannot be autocorrected.

@safety
  The cop autocorrects regexp character classes
  by replacing one character range with two: `A-z` becomes `A-Za-z`.
  In most cases this is probably what was originally intended
  but it changes the regexp to no longer match symbols it used to include.
  For this reason, this cop's autocorrect is unsafe (it will
  change the behavior of the code).

```ruby
# bad
r = /[A-z]/

# good
r = /[A-Za-z]/
```

## Options

This rule has no options.

## Blind spots

Ranges inside a nested character class (`[a-[A-Z]]`-style set subtraction/intersection) are not checked; the nested bracket is treated as one opaque token, matching `Style/RedundantRegexpCharacterClass`'s own simplification. No fixture exercises a range inside one.

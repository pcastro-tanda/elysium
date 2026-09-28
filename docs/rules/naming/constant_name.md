# Naming/ConstantName

Checks whether constant names are written using SCREAMING_SNAKE_CASE.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks whether constant names are written using
SCREAMING_SNAKE_CASE.

To avoid false positives, it ignores cases in which we cannot know
for certain the type of value that would be assigned to a constant.

```ruby
# bad
InchInCm = 2.54
INCHinCM = 2.54
Inch_In_Cm = 2.54

# good
INCH_IN_CM = 2.54
```

## Options

This rule has no options.

## Blind spots

`[[:digit:][:upper:]_]` is POSIX's character classes, matched here with Rust's Unicode-aware
`char::is_uppercase`/`is_ascii_digit`; this agrees with upstream on every accented Latin letter
its own doc comment calls out, but is not a byte-for-byte match of Oniguruma's POSIX `[:upper:]`
table for every script.

# Lint/CopDirectiveSyntax

Checks that `# rubocop:` directives are strictly formatted.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | nursery |

Checks that `# rubocop:enable ...` and `# rubocop:disable ...` statements are strictly formatted.

A comment can be added to the directive by prefixing it with `--`.

```ruby
# bad
# rubocop:disable Layout/LineLength Style/Encoding

# good
# rubocop:disable Layout/LineLength, Style/Encoding

# bad
# rubocop:disable

# good
# rubocop:disable all

# bad - only the first directive takes effect
# rubocop:disable Layout/LineLength # rubocop:disable Style/Encoding

# good
# rubocop:disable Layout/LineLength, Style/Encoding

# bad
# rubocop:wrongmode Layout/LineLength

# good
# rubocop:disable Layout/LineLength

# bad
# rubocop:disable Layout/LineLength comment

# good
# rubocop:disable Layout/LineLength -- comment

# bad
# rucocop:disable Layout/LineLength

# good
# rubocop:disable Layout/LineLength

# bad
# rubocop:disable Layout/LineLenght

# good
# rubocop:disable Layout/LineLength
```

## Options

This rule has no options.

## Blind spots

Resolving an unqualified cop name (e.g. `# rubocop:disable LineLength`) picks any known cop sharing that bare name instead of raising on an ambiguous match like RuboCop's registry would; the built-in registry never has two cops sharing a bare name, so this never actually differs.

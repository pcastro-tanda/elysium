# Rails/EnumUniqueness

Avoid duplicate integers in hash-syntax `enum` declaration.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Looks for duplicate values in enum declarations.

```ruby
# bad
enum :status, { active: 0, archived: 0 }

# good
enum :status, { active: 0, archived: 1 }

# bad
enum :status, [:active, :archived, :active]

# good
enum :status, [:active, :archived]

# bad
enum status: { active: 0, archived: 0 }

# good
enum status: { active: 0, archived: 1 }

# bad
enum status: [:active, :archived, :active]

# good
enum status: [:active, :archived]
```

## Options

This rule has no options.

## Blind spots

Duplicates are found by comparing literal values (strings and symbols by content, everything else by source text), where RuboCop compares ASTs; `1` and `0x1` are therefore different.

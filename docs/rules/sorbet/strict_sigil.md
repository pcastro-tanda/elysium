# Sorbet/StrictSigil

All files must be at least at strictness `strict`.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Makes the Sorbet `strict` sigil mandatory in all files.

```ruby
# bad
# typed: true

# bad
# typed: false

# good
# typed: strict
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| SuggestedStrictness | `strict` |  | Sorbet strictness level suggested in offense messages and used in autocorrect. |
| ExactStrictness | `nil` |  | If set, make offense if the strictness level in the file is different than this one. |

## Blind spots

None recorded.

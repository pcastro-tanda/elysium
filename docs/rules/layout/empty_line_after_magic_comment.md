# Layout/EmptyLineAfterMagicComment

Checks for a newline after the final magic comment.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Add an empty line after magic comments to separate them from the code.

`NumberOfEmptyLines` configures the minimum number of empty lines required.
Set it to `2` when using YARD, which otherwise treats the magic comments as
documentation for the first module or class in the file.

NOTE: `Layout/EmptyLines` has to be disabled for values greater than `1`, as
it removes the extra empty lines this cop adds, and autocorrecting with both
enabled loops between them.

```ruby
# good
# frozen_string_literal: true

# Some documentation for Person
class Person
  # Some code
end

# bad
# frozen_string_literal: true
# Some documentation for Person
class Person
  # Some code
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| NumberOfEmptyLines | 1 |  | The minimum number of empty lines required after magic comments. |

## Blind spots

`RuboCop::MagicComment` is reimplemented against raw comment text rather
than upstream's parser-gem-backed wrapper classes. Every keyword
(`frozen_string_literal`, `encoding`/`coding`, `rbs_inline`, `warn_indent`,
`shareable_constant_value`, `typed`) and all three comment syntaxes (plain,
Emacs `-*- ... -*-`, Vim `# vim: ...`) are ported, including the
`rbs_inline` restriction to a literal `enabled`/`disabled` value and the
case-sensitivity difference between the plain-comment patterns (RuboCop's
`/io` flag) and the Emacs/Vim ones (no flag).

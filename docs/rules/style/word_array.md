# Style/WordArray

Use %w or %W for arrays of words.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

Checks for array literals made up of word-like strings, that are not using
the `%w()` syntax.

Alternatively, it can check for uses of the `%w()` syntax, in projects which
do not want to include that syntax.

NOTE: When using the `percent` style, `%w()` arrays containing a space will
be registered as offenses.

The `MinSize` configuration option causes the cop to be ignored for arrays
smaller than the given value: a `MinSize` of `3` will not enforce a style on
an array of 2 or fewer elements.

```ruby
# EnforcedStyle: percent (default)

# good
%w[foo bar baz]

# bad
['foo', 'bar', 'baz']

# bad (contains spaces)
%w[foo\ bar baz\ quux]

# bad
[
  ['one', 'One'],
  ['two', 'Two']
]

# good
[
  %w[one One],
  %w[two Two]
]

# good (2d array containing spaces)
[
  ['one', 'One'],
  ['two', 'Two'],
  ['forty two', 'Forty Two']
]
```

```ruby
# EnforcedStyle: brackets

# good
['foo', 'bar', 'baz']

# bad
%w[foo bar baz]

# good (contains spaces)
['foo bar', 'baz quux']

# good
[
  ['one', 'One'],
  ['two', 'Two']
]

# bad
[
  %w[one One],
  %w[two Two]
]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `percent` | `percent`, `brackets` | Whether word arrays should use `%w`/`%W` or `[...]` literals. |
| MinSize | 2 |  | Arrays with fewer elements than this are ignored. |
| WordRegex | `\A(?:\p{Word}|\p{Word}-\p{Word}|\n|\t)+\z` |  | Pattern each (unescaped) element must fully match to count as a `word`. |

## Blind spots

`--auto-gen-config` bookkeeping (`array_style_detected`/`no_acceptable_style!`,
tracking the smallest percent array and largest bracket array seen across an
entire run to decide a project-wide style or disable the cop) is not ported:
it only affects `rubocop --auto-gen-config` output, never a single file's own
offenses, and this rule lints one file at a time.

`to_string_literal`'s encoding-aware branch assumes the process's
`Encoding.default_external` is UTF-8 (the overwhelmingly common case); RuboCop
itself derives this from the running Ruby's environment, which a per-file
linter has no equivalent of and no configuration knob for. A `WordRegex`
override written in Oniguruma syntax beyond a bare `(?-mix:...)` wrapper (or
using named classes other than `\p{Word}`, e.g. POSIX bracket names) is used
as-is after that one substitution and may not compile or match identically in
the `regex` crate. `to_string_literal`'s rare invalid-external-encoding
fallback (backslash-doubling before re-quoting) is simplified to a plain
requote, since no fixture exercises it. `PercentArray#invalid_percent_array_context?`
is intentionally unported; see the module doc comment.

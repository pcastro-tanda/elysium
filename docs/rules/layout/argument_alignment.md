# Layout/ArgumentAlignment

Align the arguments of a method call if they span more than one line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

Checks that the arguments on a multi-line method call are aligned.

```ruby
# EnforcedStyle: with_first_argument (default)

# good

foo :bar,
    :baz,
    key: value

foo(
  :bar,
  :baz,
  key: value
)

# bad

foo :bar,
  :baz,
  key: value

foo(
  :bar,
    :baz,
    key: value
)
```

```ruby
# EnforcedStyle: with_fixed_indentation

# good

foo :bar,
  :baz,
  key: value

# bad

foo :bar,
    :baz,
    key: value
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `with_first_argument` | `with_first_argument`, `with_fixed_indentation` | Aligns following lines with the first argument (`with_first_argument`) or one indentation level past the method call's own line (`with_fixed_indentation`). |
| IndentationWidth | `nil` |  | Overrides `Layout/IndentationWidth`'s configured width for `with_fixed_indentation`'s base column; falls back to it, else 2. |

## Blind spots

`display_column` approximates Ruby's `unicode-display_width` gem with a
hand-rolled East Asian Width table covering the common CJK, Hangul, and
fullwidth-forms ranges; combining marks, emoji sequences, and rarer wide
code points are not modeled and could misalign a comparison in exotic
source files.

Autocorrection's taboo-range protection (RuboCop's `AlignmentCorrector`
`inside_string_ranges`) only covers heredoc bodies; the interior of an
ordinary multi-line quoted string or `%`-literal that itself begins a
physical line inside a misaligned argument is not separately protected.
The block-comment guard is a per-line `=begin` text match rather than
resolving actual `EmbDoc` comment nodes, matching this crate's other
`Alignment`-based cops.

`autocorrect_incompatible_with_other_cops?` (a `with_first_argument`-style
conflict with `Layout/HashAlignment` configured for a `separator` alignment
style) is honoured through `RuleOptions::peer`, which only sees `Layout/
HashAlignment` options explicitly set in the loaded config, not that cop's
own defaults merged in; since the default `EnforcedHashRocketStyle`/
`EnforcedColonStyle` is `key`, not `separator`, this matches RuboCop's
actual behaviour for every config that does not explicitly opt in.

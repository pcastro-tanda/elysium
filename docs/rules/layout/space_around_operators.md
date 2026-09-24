# Layout/SpaceAroundOperators

Checks that operators have space around them, except for ** which should or shouldn't have surrounding space depending on configuration.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

It allows vertical alignment consisting of one or more whitespace around operators.

This cop has `AllowForAlignment` option. When `true`, allows most uses of extra spacing if the
intent is to align with an operator on the previous or next line, not counting empty lines or
comment lines.

```ruby
# bad
total = 3*4
"apple"+"juice"
my_number = 38/4

# good
total = 3 * 4
"apple" + "juice"
my_number = 38 / 4
```

`EnforcedStyleForExponentOperator: no_space` (default) requires `a**b`; `space` requires
`a ** b`.

`EnforcedStyleForRationalLiterals: no_space` (default) requires `1/48r`; `space` requires
`1 / 48r`.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowForAlignment | true |  | Allows extra spacing when it lines an operator up with one on an adjacent line. |
| EnforcedStyleForExponentOperator | `no_space` | `space`, `no_space` | Whether `**` should have surrounding space. |
| EnforcedStyleForRationalLiterals | `no_space` | `space`, `no_space` | Whether `/` should have surrounding space when dividing by a rational literal. |

## Blind spots

`AllowForAlignment`'s adjacent-line heuristics (RuboCop's `PrecedingFollowingAlignment` mixin) are
approximated with raw byte scans over `ctx.line_text` instead of a real token stream: operator/
assignment-token detection (`aligned_equals_operator?`, the per-file `assignment_lines` set feeding
`aligned_with_equals_sign`) does not skip string/comment contents, so a `=`/comparison-like
sequence inside a string literal on a candidate line can be mistaken for a real token. Any such
mismatch only ever grants extra alignment leniency (suppressing a real offense), never fabricates
one. `Layout/HashAlignment`'s `AllowMultipleStyles`/array-of-styles interaction beyond checking
whether `table` appears in `EnforcedHashRocketStyle` is not modeled (RuboCop reads the raw
`Array(...)` the same way). `TargetRubyVersion`-gating of `Layout/SpaceAroundOperators`'s one-line
`=>` pattern-matching check (RuboCop skips it below Ruby 3.0) is not implemented; this rule always
checks it, matching the common case where pattern matching is enabled at all.

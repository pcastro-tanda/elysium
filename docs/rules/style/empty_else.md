# Style/EmptyElse

Checks for empty `else`-clauses, possibly including comments and/or an explicit `nil` depending on the `EnforcedStyle`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: both (default)
# warn on empty else and else with nil in it

# bad
if condition
  statement
else
  nil
end

# bad
if condition
  statement
else
end

# good
if condition
  statement
else
  statement
end

# good
if condition
  statement
end
```

With `EnforcedStyle: empty`, only a completely empty `else`-clause is flagged
(an explicit `nil` is allowed). With `EnforcedStyle: nil`, only an
`else`-clause whose sole statement is `nil` is flagged (a completely empty
one is allowed).

With `AllowComments: true`, an `else`-clause that carries a comment --
whether or not it is otherwise empty or `nil` -- is never flagged.

`if`, `unless`, and `case` are covered; `case`/`in` pattern matching is not
(see `blind_spots`).

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `both` | `empty`, `nil`, `both` | `empty` flags a completely empty `else`; `nil` flags an `else` whose sole statement is `nil`; `both` flags either. |
| AllowComments | false |  | Allow a comment inside an otherwise-empty/`nil` `else`-clause. |

## Blind spots

`case`/`in` pattern matching (Prism's `CaseMatchNode`) is never checked:
RuboCop's cop only defines `on_normal_if_unless` and `on_case`, so
`case/in` expressions never reach it (pattern matching already raises
`NoMatchingPatternError` without an `else`, per the cop's own class
comment) -- this is a deliberate false negative, not an oversight.

`Style/MissingElse`'s `EnforcedStyle` (read as a peer option, to decide
whether autocorrection is forbidden when that peer cop is enabled) always
resolves through this engine's `LoadedConfig`, which merges every cop's
settings with the bundled real `config/default.yml` -- so `EnforcedStyle`
reads as "both" (RuboCop's own documented default) whenever a fixture's
`.rubocop.yml` enables `Style/MissingElse` without naming `EnforcedStyle`
explicitly. RuboCop's own cop-spec unit tests build a bare
`RuboCop::Config.new(hash)` with no such default merge, so the same
`{'Enabled' => true}` peer hash resolves `EnforcedStyle` to `nil` there
instead, never forbidding autocorrection. This is a structural mismatch
between the fixtures' RSpec-derived ground truth and this engine's (more
realistic, CLI-accurate) config resolution, not something `configure`
can distinguish through the `RuleOptions::peer` API -- it affects the
`autocorrect_missingelse_is_disabled_does_autocorrection*` fixtures only.

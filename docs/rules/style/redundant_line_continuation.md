# Style/RedundantLineContinuation

Checks for redundant line continuation.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

A line continuation is redundant when removing the backslash does not change how the program parses: the source is reparsed without the backslash and the resulting AST is compared to the original. Only backslashes that are pure noise are reported; backslashes that are significant -- inside strings, for string concatenation, before an operator or argument that would otherwise start a new statement, and so on -- are left alone, as are backslashes in comments.

## Options

This rule has no options.

## Blind spots

Candidates sharing a method/class/module scope are verified one at a time instead of upstream's batch-then-fallback reparse grouping; a set of backslashes that only parses equivalently when removed *together* (but not individually) would be missed here. No fixture exercises this. The `parser`
gem's leading-dot/blank-line special case is intentionally not ported (see module doc): Prism's own grammar does not share that quirk.

fixtures/style/redundant_line_continuation/does_not_register_an_offense_when_a_line_continuation_prec_2.rb
is unfixable as generated: its upstream spec is an `expect_no_offenses` over
a four-line backslash-continued `1 & 2 | 3 ^ 4` expression, but the fixture
harness's own annotation parser treats the literal fourth line (`^ 4`,
Ruby's bitwise-xor operator applied to 4) as a caret offense-annotation line
and strips it, so this rule only ever sees the first three lines. Given
that truncated input, the final backslash genuinely is redundant (it
dangles at true end-of-file with nothing left to continue into, so removing
it reparses identically) -- the same verdict upstream's own
`verified_by_reparse` would reach on this mangled input. This is a
fixture-generation artifact, not a cop defect.

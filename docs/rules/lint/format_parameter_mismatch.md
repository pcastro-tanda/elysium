# Lint/FormatParameterMismatch

Checks for a mismatch between the number of expected fields for format/sprintf/#% and what is actually passed as arguments.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for a mismatch between the number of expected fields for
format/sprintf/#% and what is actually passed as arguments.

In addition, it checks whether different formats are used in the same
format string. Do not mix numbered, unnumbered, and named formats in
the same format string.

```ruby
# bad
format('A value: %s and another: %i', a_value)

# good
format('A value: %s and another: %i', a_value, another)

# bad
format('Unnumbered format: %s and numbered: %2$s', a_value, another)

# good
format('Numbered format: %1$s and numbered %2$s', a_value, another)
```

## Options

This rule has no options.

## Blind spots

`node.receiver == value_node.receiver`-style structural equality is not needed by this cop, but
two genuine simplifications are: (1) `FormatString#max_digit_dollar_num`'s all-sequences
`Array#max` (which can raise `ArgumentError` in real RuboCop for a numbered format string that
also contains a `%%` escape, untested by any fixture) is replaced by a max over only the
sequences that have a digit-dollar number, never crashing and behaviour-identical for every
string `mixed_formats?` lets through; (2) `heredoc?`'s raw `source[0, 2] == '<<'` prefix check
(meaningful only against whitequark's marker-only heredoc `#source`) is replaced by a heredoc
node-kind check, since Prism's heredoc span covers the full multi-line body -- behaviour-identical
for every non-heredoc case (every case this cop's own spec exercises) and, unlike the prefix hack,
actually recognizes a genuine heredoc format string.

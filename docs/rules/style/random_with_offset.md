# Style/RandomWithOffset

Prefer to use ranges when generating random numbers instead of integers with offsets.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |



## Options

This rule has no options.

## Blind spots

Offsets and range endpoints are only recognized as bare integer literals fitting in an `i32`, matching upstream's `def_node_matcher` patterns (`(int $_)`); an offset held in a variable, produced by another expression, or too large for `i32` is not flagged.

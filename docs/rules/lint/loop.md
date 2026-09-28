# Lint/Loop

Use Kernel#loop with break rather than begin/end/until or begin/end/while for post-loop tests.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for uses of `begin...end while/until something`. `Kernel#loop` with `break` should be preferred, since the behaviour of `begin/end` post-condition loops (running the body at least once, even when the condition is already false) is a frequent source of confusion.

## Options

This rule has no options.

## Blind spots

None recorded.

# Layout/SpaceInsideArrayPercentLiteral

No unnecessary additional spaces between elements in %i/%w literals.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for unnecessary additional spaces inside array percent literals (i.e. %i/%w).

Note that blank percent literals (e.g. `%i( )`) are checked by `Layout/SpaceInsidePercentLiteralDelimiters`.

## Options

This rule has no options.

## Blind spots

The escaped-space scan is a hand-rolled stand-in for a lookaround regex Rust's engine can't express; it always resolves each candidate start byte with the same one choice the real regex's backtracking converges on for ordinary `%w`/`%i` items (maximal `\ ` consumption, then the longest immediately-following space run), rather than exploring every backtrack path, so a contrived item mixing runs of escaped and unescaped spaces in a way that requires giving back consumed `\ \ ` pairs to find a match could be scored differently.

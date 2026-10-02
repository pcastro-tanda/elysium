# Style/EmptyHeredoc

Checks for using empty heredoc to reduce redundancy.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |



## Options

This rule has no options.

## Blind spots

Reads `Style/StringLiterals`'s `EnforcedStyle` as a peer option to pick the replacement literal's quote style.

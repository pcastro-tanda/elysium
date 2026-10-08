# Style/RedundantCurrentDirectoryInPath

Checks for a redundant current directory in a path given to `require_relative`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for paths given to `require_relative` that start with the current directory (`./`), which can be omitted.

## Options

This rule has no options.

## Blind spots

None recorded.

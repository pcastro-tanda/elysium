# Style/RedundantStringEscape

Checks for redundant escapes in string literals.

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

A `%W`/interpolated-array element that is itself a multi-part interpolated string only inherits the array's delimiter/percent-array context one container deep; a plain-text run nested two containers below the array is not covered.

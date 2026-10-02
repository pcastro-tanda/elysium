# Style/RedundantRegexpArgument

Identifies places where argument can be replaced from a deterministic regexp to a string.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where argument can be replaced from a deterministic regexp to a string.

## Options

This rule has no options.

## Blind spots

Interpolated regexp literals (`InterpolatedRegularExpressionNode`) are
accepted for the options check but never produce a replacement string: their
content is not reassembled from parts (no fixture exercises a deterministic
interpolated-regexp first argument, since any `#{...}`/`{`/`}` text fails the
upstream `DETERMINISTIC_REGEX` character class anyway).

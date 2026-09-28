# Lint/InterpolationCheck

Checks for interpolation in a single quoted string.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for interpolation in a single quoted string. A single-quoted string that happens to contain `#{...}` never actually interpolates; this usually means the author meant to use a double-quoted string.

## Options

This rule has no options.

## Blind spots

`node.parent&.regexp_type?` is unreachable under Prism: regexp literals are never composed of `StringNode`s, so it is not ported.

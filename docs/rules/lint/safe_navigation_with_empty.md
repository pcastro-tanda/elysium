# Lint/SafeNavigationWithEmpty

Avoid `foo&.empty?` in conditionals.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks to make sure safe navigation isn't used with `empty?` in a conditional.

While the safe navigation operator is generally a good idea, when checking `foo&.empty?` in a conditional, `foo` being `nil` will actually do the opposite of what the author intends.

## Options

This rule has no options.

## Blind spots

None recorded.

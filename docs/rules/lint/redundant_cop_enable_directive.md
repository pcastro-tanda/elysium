# Lint/RedundantCopEnableDirective

Checks for rubocop:enable comments that can be removed.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Detects instances of `rubocop:enable` comments that can be removed.

When comment enables all cops at once `rubocop:enable all` that cop checks whether any cop was actually enabled.

```ruby
# bad
foo = 1
# rubocop:enable Layout/LineLength

# good
foo = 1

# bad
# rubocop:disable Style/StringLiterals
foo = "1"
# rubocop:enable Style/StringLiterals
baz
# rubocop:enable all

# good
# rubocop:disable Style/StringLiterals
foo = "1"
# rubocop:enable all
baz
```

## Options

This rule has no options.

## Blind spots

`-next` directives, out of `ruby_directives`' scope, never count as a disable or a redundant enable here.

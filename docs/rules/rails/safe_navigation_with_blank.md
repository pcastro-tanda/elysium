# Rails/SafeNavigationWithBlank

Avoid `foo&.blank?` in conditionals.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks to make sure safe navigation isn't used with `blank?` in a conditional.

While the safe navigation operator is generally a good idea, when checking `foo&.blank?` in a conditional, `foo` being `nil` will actually do the opposite of what the author intends: `foo&.blank?` is `nil` whereas `foo.blank?` is `true`.

```ruby
# bad
do_something if foo&.blank?
do_something unless foo&.blank?

# good
do_something if foo.blank?
do_something unless foo.blank?
```

## Options

This rule has no options.

## Blind spots

None recorded.

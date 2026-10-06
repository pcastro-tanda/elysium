# Rails/BelongsTo

Use `optional: true` instead of `required: false` for `belongs_to` relations.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Looks for `belongs_to` associations where we control whether the association is required via the deprecated `required` option instead.

Since Rails 5, `belongs_to` associations are required by default and this can be controlled through the use of `optional: true`.

`required: false` is corrected to `optional: true`; `required: true` is inverted to `optional: false`, which the user may then remove depending on their defaults.

```ruby
# bad
belongs_to :blog, required: false

# good
belongs_to :blog, optional: true

# bad
belongs_to :blog, required: true

# good
belongs_to :blog, optional: false
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.

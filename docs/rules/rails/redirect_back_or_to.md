# Rails/RedirectBackOrTo

Use `redirect_back_or_to` instead of `redirect_back` with `fallback_location` option.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Prefer `redirect_back_or_to` over `redirect_back` with a `fallback_location` keyword argument (Rails 7.0+).

```ruby
# bad
redirect_back(fallback_location: root_path)

# good
redirect_back_or_to(root_path)
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; the locked `railties` version from `Gemfile.lock` is used when no `TargetRailsVersion` is set.

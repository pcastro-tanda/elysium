# Rails/EnvironmentComparison

Favor `Rails.env.production?` over `Rails.env == 'production'`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that `Rails.env` is compared using `.production?`-like methods instead of equality against a string or symbol.

```ruby
# bad
Rails.env == 'production'
Rails.env.to_sym == :production

# bad, always returns false
Rails.env == :test

# good
Rails.env.production?
```

## Options

This rule has no options.

## Blind spots

None recorded.

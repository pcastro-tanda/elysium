# Rails/DelegateAllowBlank

Do not use allow_blank as an option to delegate.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Looks for delegations that pass :allow_blank as an option instead of :allow_nil. :allow_blank is not a valid option to pass to ActiveSupport#delegate.

```ruby
# bad
delegate :foo, to: :bar, allow_blank: true

# good
delegate :foo, to: :bar, allow_nil: true
```

## Options

This rule has no options.

## Blind spots

None recorded.

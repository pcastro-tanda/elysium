# Rails/AttributeDefaultBlockValue

Pass method call in block for attribute option `default`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Looks for `attribute` class methods that specify a `:default` option which value is an array, string literal or method call without a block. It will accept all other values, such as string, symbol, integer and float literals as well as constants.

```ruby
# bad
attribute :confirmed_at, :datetime, default: Time.zone.now

# good
attribute :confirmed_at, :datetime, default: -> { Time.zone.now }

# bad
attribute :roles, :string, array: true, default: []

# good
attribute :roles, :string, array: true, default: -> { [] }

# good
attribute :roles, :string, array: true, default: [].freeze

# bad
attribute :configuration, default: {}

# good
attribute :configuration, default: -> { {} }

# good
attribute :role, :string, default: :customer
```

## Options

This rule has no options.

## Blind spots

None recorded.

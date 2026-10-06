# Rails/RedundantAllowNil

Finds redundant use of `allow_nil` when `allow_blank` is set to certain values in model validations.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks Rails model validations for a redundant `allow_nil` when `allow_blank` is present.

```ruby
# bad
validates :x, length: { is: 5 }, allow_nil: true, allow_blank: true

# bad
validates :x, length: { is: 5 }, allow_nil: false, allow_blank: true

# bad
validates :x, length: { is: 5 }, allow_nil: false, allow_blank: false

# good
validates :x, length: { is: 5 }, allow_blank: true

# good
validates :x, length: { is: 5 }, allow_blank: false

# good
# Here, `nil` is valid but `''` is not
validates :x, length: { is: 5 }, allow_nil: true, allow_blank: false
```

## Options

This rule has no options.

## Blind spots

Whether the two values have the same type is judged on Prism node kinds mapped onto whitequark's types.

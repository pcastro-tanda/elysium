# Lint/NestedPercentLiteral

Checks for nested percent literals.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for nested percent literals.

```ruby
# bad

# The percent literal for nested_attributes is parsed as four tokens,
# yielding the array [:name, :content, :"%i[incorrectly", :"nested]"].
attributes = {
  valid_attributes: %i[name content],
  nested_attributes: %i[name content %i[incorrectly nested]]
}

# good

# Neither is incompatible with the bad case, but probably the intended code.
attributes = {
  valid_attributes: %i[name content],
  nested_attributes: [:name, :content, %i[incorrectly nested]]
}

attributes = {
  valid_attributes: %i[name content],
  nested_attributes: [:name, :content, [:incorrectly, :nested]]
}
```

## Options

This rule has no options.

## Blind spots

Only plain (non-interpolated) `%i`/`%w` elements are checked, matching upstream's own
`children.first.to_s` shape for a Ruby `str`/`sym` node; see the module doc for why an
interpolated element (one containing `#{`) can never match either implementation.

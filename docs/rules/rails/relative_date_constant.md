# Rails/RelativeDateConstant

Do not assign relative date to constants.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks whether constant value isn't relative date.
Because the relative date will be evaluated only once.

This cop's autocorrection is unsafe because the replaced method is called on each reference, which may be slower than reading a constant.

```ruby
# bad
class SomeClass
  EXPIRED_AT = 1.week.since
end

# good
class SomeClass
  def self.expired_at
    1.week.since
  end
end
```

## Options

This rule has no options.

## Blind spots

None recorded.

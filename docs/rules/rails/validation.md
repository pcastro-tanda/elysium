# Rails/Validation

Use validates :attribute, hash of validations.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for the use of old-style attribute validation macros.

```ruby
# bad
validates_presence_of :foo

# good
validates :foo, presence: true
```

## Options

This rule has no options.

## Blind spots

None recorded.

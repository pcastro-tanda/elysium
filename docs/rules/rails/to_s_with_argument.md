# Rails/ToSWithArgument

Identifies passing any argument to `#to_s`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies passing any argument to `#to_s`.

This cop is marked as unsafe because it may detect `#to_s` calls that are not related to Active Support implementation.

```ruby
# bad
obj.to_s(:delimited)

# good
obj.to_formatted_s(:delimited)
```

## Options

This rule has no options.

## Blind spots

None recorded.

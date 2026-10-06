# Rails/ReflectionClassName

Use a string for `class_name` option value in the definition of a reflection.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks if the value of the option `class_name`, in the definition of a reflection is a string.

This cop is unsafe because it cannot be determined whether constant or method return value specified to `class_name` is a string.

```ruby
# bad
has_many :accounts, class_name: Account
has_many :accounts, class_name: Account.name

# good
has_many :accounts, class_name: 'Account'
```

## Options

This rule has no options.

## Blind spots

None recorded.

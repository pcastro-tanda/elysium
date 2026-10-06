# Rails/ActiveRecordAliases

Avoid Active Record aliases: Use `update` instead of `update_attributes`. Use `update!` instead of `update_attributes!`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks that ActiveRecord aliases are not used. The direct method names are more clear and easier to read.

This cop is unsafe because custom `update_attributes` method call was changed to `update` but the method name remained same in the method definition.

```ruby
# bad
book.update_attributes!(author: 'Alice')

# good
book.update!(author: 'Alice')
```

## Options

This rule has no options.

## Blind spots

None recorded.

# Rails/RootJoinChain

Use a single `#join` instead of chaining on `Rails.root` or `Rails.public_path`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Use a single `#join` instead of chaining on `Rails.root` or `Rails.public_path`.

```ruby
# bad
Rails.root.join('db').join('schema.rb')
Rails.public_path.join('path').join(to).join('file.pdf')

# good
Rails.root.join('db', 'schema.rb')
Rails.public_path.join('path', to, 'file.pdf')
```

## Options

This rule has no options.

## Blind spots

None recorded.

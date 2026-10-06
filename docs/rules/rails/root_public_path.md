# Rails/RootPublicPath

Favor `Rails.public_path` over `Rails.root` with `'public'`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Favor `Rails.public_path` over `Rails.root` with `'public'`.

```ruby
# bad
Rails.root.join('public')
Rails.root.join('public/file.pdf')
Rails.root.join('public', 'file.pdf')

# good
Rails.public_path
Rails.public_path.join('file.pdf')
Rails.public_path.join('file.pdf')
```

## Options

This rule has no options.

## Blind spots

None recorded.

# Rails/RootPathnameMethods

Use `Rails.root` IO methods instead of passing it to `File`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Use `Rails.root` IO methods instead of passing it to `File`.

`Rails.root` is an instance of `Pathname` so we can apply many IO methods directly.

This cop works best when used together with `Style/FileRead`, `Style/FileWrite` and `Rails/RootJoinChain`.

This cop is unsafe for autocorrection because ``Dir``'s `children`, `each_child`, `entries`, and `glob` methods return string element, but these methods of `Pathname` return `Pathname` element.

```ruby
# bad
File.open(Rails.root.join('db', 'schema.rb'))
File.read(Rails.root.join('db', 'schema.rb'))
Dir.glob(Rails.root.join('db', 'schema.rb'))

# good
Rails.root.join('db', 'schema.rb').open
Rails.root.join('db', 'schema.rb').read
Rails.root.glob("db/schema.rb")
```

## Options

This rule has no options.

## Blind spots

`Dir.glob(Rails.root)` (no `join`), which makes RuboCop raise, reports nothing.

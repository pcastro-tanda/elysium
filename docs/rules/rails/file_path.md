# Rails/FilePath

Use `Rails.root.join` for file path joining.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies usages of file path joining process to use `Rails.root.join` clause. It is used to add uniformity when joining paths.

NOTE: This cop ignores leading slashes in string literal arguments for `Rails.root.join` and multiple slashes in string literal arguments for `Rails.root.join` and `File.join`.

```ruby
# EnforcedStyle: slashes (default)
# bad
Rails.root.join('app', 'models', 'goober')

# good
Rails.root.join('app/models/goober')

# bad
File.join(Rails.root, 'app/models/goober')
"#{Rails.root}/app/models/goober"

# good
Rails.root.join('app/models/goober').to_s
```

```ruby
# EnforcedStyle: arguments
# bad
Rails.root.join('app/models/goober')

# good
Rails.root.join('app', 'models', 'goober')

# bad
File.join(Rails.root, 'app/models/goober')
"#{Rails.root}/app/models/goober"

# good
Rails.root.join('app', 'models', 'goober').to_s
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `slashes` | `slashes`, `arguments` | Whether `Rails.root.join` takes one slash-separated path or one argument per path segment. |

## Blind spots

None recorded.

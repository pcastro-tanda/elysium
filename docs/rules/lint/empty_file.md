# Lint/EmptyFile

Enforces that Ruby source files are not empty.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Enforces that Ruby source files are not empty.

```ruby
# bad
# Empty file

# good
# File containing non commented source lines
```

```ruby
# AllowComments: true (default)
# good
# File consisting only of comments
```

```ruby
# AllowComments: false
# bad
# File consisting only of comments
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowComments | true |  | Allow files that consist only of comments (and blank lines). |

## Blind spots

None recorded.

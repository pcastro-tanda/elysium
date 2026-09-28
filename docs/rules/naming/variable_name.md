# Naming/VariableName

Makes sure that all variables use the configured style, snake_case or camelCase, for their names.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

```ruby
# EnforcedStyle: snake_case (default)

# bad
fooBar = 1

# good
foo_bar = 1
```

```ruby
# EnforcedStyle: camelCase

# bad
foo_bar = 1

# good
fooBar = 1
```

```ruby
# AllowedIdentifiers: ['fooBar']

# good (with EnforcedStyle: snake_case)
fooBar = 1
```

```ruby
# AllowedPatterns: ['_v\d+\z']

# good (with EnforcedStyle: camelCase)
release_v1 = true
```

```ruby
# ForbiddenIdentifiers: ['fooBar']

# bad
fooBar = 1
```

```ruby
# ForbiddenPatterns: ['_v\d+\z']

# bad
release_v1 = true
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `snake_case` | `snake_case`, `camelCase` | Naming style variables must follow. |
| AllowedIdentifiers | `[]` |  | Variable names (sigils stripped) that are never checked. |
| AllowedPatterns | `[]` |  | Regexps; a variable name matching one is accepted regardless of style. |
| ForbiddenIdentifiers | `[]` |  | Variable names (sigils stripped) that are always flagged. |
| ForbiddenPatterns | `[]` |  | Regexps; a variable name matching one is always flagged. |

## Blind spots

`it` and numbered block parameters (`_1`) are read through dedicated Prism nodes and are never checked; both always satisfy `snake_case`.

# Naming/MethodName

Makes sure that all methods use the configured style, snake_case or camelCase, for their names.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | nursery |

Method names matching `AllowedPatterns` are always allowed, and
`ForbiddenIdentifiers`/`ForbiddenPatterns` are always flagged. Operator
methods are never checked.

```ruby
# EnforcedStyle: snake_case (default)

# bad
def fooBar; end

# good
def foo_bar; end
```

```ruby
# EnforcedStyle: camelCase

# bad
def foo_bar; end

# good
def fooBar; end
```

```ruby
# ForbiddenIdentifiers: ['def', 'super']

# bad
def def; end
def super; end
```

```ruby
# ForbiddenPatterns: ['_v1\z', '_gen1\z']

# bad
def release_v1; end
def api_gen1; end
```

```ruby
# AllowedPatterns: ['\AonSelectionBulkChange\z']

# good
def onSelectionBulkChange(arg); end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `snake_case` | `snake_case`, `camelCase` | Naming style method names must follow. |
| AllowedPatterns | `[]` |  | Regexps; a method name matching one is never checked. |
| ForbiddenIdentifiers | `__id__`, `__send__` |  | Method names that are always flagged. |
| ForbiddenPatterns | `[]` |  | Regexps; a method name matching one is always flagged. |

## Blind spots

None recorded.

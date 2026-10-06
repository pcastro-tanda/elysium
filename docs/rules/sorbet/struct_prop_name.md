# Sorbet/StructPropName

Checks that T::Struct property names use the configured naming style.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks that `T::Struct` property names use the configured style. The supported styles and name filters match `Naming/MethodName`.

```ruby
# bad
class User < T::Struct
  const :firstName, String
  prop :lastName, String
end

# good
class User < T::Struct
  const :first_name, String
  prop :last_name, String
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `snake_case` | `snake_case`, `camelCase` | Naming style property names must follow. |
| AllowedPatterns | `[]` |  | Regexps; a property name matching one is never checked. |
| ForbiddenIdentifiers | `[]` |  | Property names that are always flagged. |
| ForbiddenPatterns | `[]` |  | Regexps; a property name matching one is always flagged. |

## Blind spots

None recorded.

# Rails/DynamicFindBy

Use `find_by` instead of dynamic `find_by_*`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks dynamic `find_by_*` methods. Use `find_by` instead of dynamic method.

It is certainly unsafe when not configured properly, i.e. user-defined `find_by_xxx` method is not added to cop's `AllowedMethods`.

```ruby
# bad
User.find_by_name(name)
User.find_by_name_and_email(name)
User.find_by_email!(name)

# good
User.find_by(name: name)
User.find_by(name: name, email: email)
User.find_by!(email: email)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Whitelist | `find_by_sql`, `find_by_token_for` |  | Deprecated, use `AllowedMethods` instead. |
| AllowedMethods | `find_by_sql`, `find_by_token_for` |  | Dynamic finder methods that are allowed. |
| AllowedReceivers | `Gem::Specification`, `page` |  | Receivers (by source) whose dynamic finders are allowed. |

## Blind spots

None recorded.

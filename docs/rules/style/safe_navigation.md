# Style/SafeNavigation

Transforms usages of a method call safeguarded by a check for the existence of the object to safe navigation (`&.`). Autocorrection is unsafe as it assumes the object will be `nil` or truthy, but never `false`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Transforms usages of a method call safeguarded by a non `nil` check for the variable whose method is being called to safe navigation (`&.`). If there is a method chain, all of the methods in the chain need to be checked for safety, and all of the methods will need to be changed to use safe navigation.

# Examples

```ruby
# bad
foo.bar if foo
foo.bar.baz if foo
foo.bar(param1, param2) if foo
foo.bar { |e| e.something } if foo
foo.bar(param) { |e| e.something } if foo

foo.bar if !foo.nil?
foo.bar unless !foo
foo.bar unless foo.nil?

foo && foo.bar
foo && foo.bar.baz
foo && foo.bar(param1, param2)
foo && foo.bar { |e| e.something }
foo && foo.bar(param) { |e| e.something }

foo ? foo.bar : nil
foo.nil? ? nil : foo.bar
!foo.nil? ? foo.bar : nil
!foo ? nil : foo.bar

# good
foo&.bar
foo&.bar&.baz
foo&.bar(param1, param2)
foo&.bar { |e| e.something }
foo&.bar(param) { |e| e.something }
foo && foo.bar.baz.qux # method chain with more than 2 methods
foo && foo.nil? # method that `nil` responds to
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| ConvertCodeThatCanStartToReturnNil | false |  | Enables conversion of code such as `!foo.nil? && foo.bar` to `foo&.bar`, which as a whole can start returning `nil` in addition to what the method itself returns. |
| MaxChainLength | 2 |  | Maximum length of method chains for register an offense. |
| AllowedMethods | `present?`, `blank?`, `presence`, `try`, `try!` |  | Methods that `nil` may safely respond to for the purpose of an `&&`-chained method call, beyond `nil`'s own instance methods. |

## Blind spots

None recorded.

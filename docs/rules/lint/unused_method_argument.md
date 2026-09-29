# Lint/UnusedMethodArgument

Checks for unused method arguments.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for unused method arguments.

```ruby
# bad
def some_method(used, unused, _unused_but_allowed)
  puts used
end

# good
def some_method(used, _unused, _unused_but_allowed)
  puts used
end
```

With `AllowUnusedKeywordArguments: false` (the default), an unused keyword
argument is still flagged, just without the `_foo` rename suggestion.
With `AllowUnusedKeywordArguments: true`, unused keyword arguments are
accepted outright.

With `IgnoreEmptyMethods: true` (the default), a method with an empty body
is not flagged, even if it declares unused arguments. With
`IgnoreEmptyMethods: false`, empty methods are flagged too.

With `IgnoreNotImplementedMethods: true` (the default), a method whose
entire body is `raise NotImplementedError` (or any class named in
`NotImplementedExceptions`) or `fail ...` is not flagged. With
`IgnoreNotImplementedMethods: false`, such methods are flagged too.

A block argument that is never read is still accepted when the method's
body (or any nested block) contains a bare `yield`, since the argument is
there only to document that the method yields.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowUnusedKeywordArguments | false |  | Whether unused keyword arguments are accepted, not just exempted from the `_foo` rename suggestion. |
| IgnoreEmptyMethods | true |  | Whether to accept an unused argument in a method with an empty body. |
| IgnoreNotImplementedMethods | true |  | Whether to accept an unused argument in a method whose body only raises or fails. |
| NotImplementedExceptions | `NotImplementedError` |  | Exception class names that count as "not implemented" for `IgnoreNotImplementedMethods`. |

## Blind spots

None recorded.

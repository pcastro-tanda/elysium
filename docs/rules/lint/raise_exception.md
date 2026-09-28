# Lint/RaiseException

Checks for `raise` or `fail` statements which are raising `Exception` class.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for `raise` or `fail` statements which raise `Exception` or
`Exception.new`. Use `StandardError` or a specific exception class instead.

If you have defined your own namespaced `Exception` class, it is possible
to configure the cop to allow it by setting `AllowedImplicitNamespaces` to
an array with the names of the namespaces to allow. By default, this is set to
`['Gem']`, which allows `Gem::Exception` to be raised without an explicit
namespace. If not allowed, a false positive may be registered if
`raise Exception` is called within the namespace.

Alternatively, use a fully qualified name with `raise`/`fail`
(eg. `raise Namespace::Exception`).

```ruby
# bad
raise Exception, 'Error message here'
raise Exception.new('Error message here')

# good
raise StandardError, 'Error message here'
raise MyError.new, 'Error message here'
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedImplicitNamespaces | `Gem` |  | Allows `Exception` to be raised bare inside these module namespaces. |

## Blind spots

None recorded.

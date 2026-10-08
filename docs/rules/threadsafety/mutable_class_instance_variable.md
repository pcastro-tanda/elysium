# ThreadSafety/MutableClassInstanceVariable

Do not assign mutable objects to class instance variables.

| | |
| --- | --- |
| Department | ThreadSafety |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks whether some class instance variable isn't a mutable literal (e.g.
array or hash).

It is based on Style/MutableConstant from RuboCop.

Class instance variables are a risk to threaded code as they are shared
between threads. A mutable object such as an array or hash may be updated via
an attr_reader so would not be detected by the
ThreadSafety/ClassAndModuleAttributes cop.

Strict mode can be used to freeze all class instance variables, rather than
just literals. Strict mode is considered an experimental feature.

```ruby
# bad
class Model
  @list = [1, 2, 3]
end

# good
class Model
  @list = [1, 2, 3].freeze
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `literals` | `literals`, `strict` | `literals` freezes literals assigned to class instance variables; `strict` freezes every assigned value. |

## Blind spots

None recorded.

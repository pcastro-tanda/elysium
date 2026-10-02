# Style/ObjectThen

Enforces the use of consistent method names `Object#yield_self` or `Object#then`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: then (default)

# bad
obj.yield_self { |x| x.do_something }

# good
obj.then { |x| x.do_something }
```

```ruby
# EnforcedStyle: yield_self

# bad
obj.then { |x| x.do_something }

# good
obj.yield_self { |x| x.do_something }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `then` | `then`, `yield_self` | Whether to prefer `Object#then` or `Object#yield_self`. |

## Blind spots

None recorded.

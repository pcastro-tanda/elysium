# Performance/BindCall

Use `bind_call(obj, args, ...)` instead of `bind(obj).call(args, ...)`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

In Ruby 2.7, `UnboundMethod#bind_call` has been added.

This cop identifies places where `bind(obj).call(args, ...)` can be replaced by `bind_call(obj, args, ...)`.

The `bind_call(obj, args, ...)` method is faster than `bind(obj).call(args, ...)`.

```ruby
# bad
umethod.bind(obj).call(foo, bar)
umethod.bind(obj).(foo, bar)

# good
umethod.bind_call(obj, foo, bar)
```

## Options

This rule has no options.

## Blind spots

None recorded.

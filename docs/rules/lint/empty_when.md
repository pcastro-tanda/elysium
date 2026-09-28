# Lint/EmptyWhen

Checks for `when` branches with empty bodies.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for the presence of `when` branches without a body.

```ruby
# bad
case foo
when bar
  do_something
when baz
end

# good
case condition
when foo
  do_something
when bar
  nil
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowComments | true |  | Allow a `when` branch to contain only comments. |

## Blind spots

None recorded.

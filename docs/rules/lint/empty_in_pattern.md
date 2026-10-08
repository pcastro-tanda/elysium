# Lint/EmptyInPattern

Checks for the presence of `in` pattern branches without a body.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for the presence of `in` pattern branches without a body.

```ruby
# bad
case condition
in [a]
  do_something
in [a, b]
end

# good
case condition
in [a]
  do_something
in [a, b]
  nil
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowComments | true |  | Allow an `in` branch to contain only comments. |

## Blind spots

None recorded.

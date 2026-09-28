# Lint/SuppressedException

Checks for `rescue` blocks with no body.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for `rescue` blocks with no body. Such empty `rescue` clauses silently swallow every exception raised in the guarded code, which almost always hides a bug rather than fixing one.

```ruby
# bad
def some_method
  do_something
rescue
end

# good
def some_method
  do_something
rescue
  handle_exception
end
```

A comment in the `rescue` body is allowed by default (`AllowComments:
true`) since it at least documents the decision to suppress the exception,
and rescuing to an explicit `nil` is allowed by default (`AllowNil: true`)
for the same reason.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowComments | true |  | Allow a comment in place of a body. |
| AllowNil | true |  | Allow rescuing to an explicit `nil` body. |

## Blind spots

None recorded.

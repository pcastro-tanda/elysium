# Lint/EmptyConditionalBody

Checks for the presence of `if`, `elsif` and `unless` branches without a body.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for the presence of `if`, `elsif` and `unless` branches without a body.

NOTE: empty `else` branches are handled by `Style/EmptyElse`.

```ruby
# bad
if condition
end

# bad
unless condition
end

# bad
if condition
  do_something
elsif other_condition
end

# good
if condition
  do_something
end

# good
unless condition
  do_something
end

# good
if condition
  do_something
elsif other_condition
  nil
end

# good
if condition
  do_something
elsif other_condition
  do_something_else
end
```

AllowComments: true (default)

```ruby
# good
if condition
  do_something
elsif other_condition
  # noop
end
```

AllowComments: false

```ruby
# bad
if condition
  do_something
elsif other_condition
  # noop
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowComments | true |  | Whether a branch containing only comments counts as empty. |

## Blind spots

None recorded.

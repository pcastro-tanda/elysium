# Lint/ShadowedArgument

Avoid reassigning arguments before they were used.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for shadowed arguments.

This cop has `IgnoreImplicitReferences` configuration option. It means argument shadowing is used in order to pass parameters to zero arity `super` when `IgnoreImplicitReferences` is `true`.

```ruby
# bad
do_something do |foo|
  foo = 42
  puts foo
end

def do_something(foo)
  foo = 42
  puts foo
end

# good
do_something do |foo|
  foo = foo + 42
  puts foo
end

def do_something(foo)
  foo = foo + 42
  puts foo
end

def do_something(foo)
  puts foo
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| IgnoreImplicitReferences | false |  | Whether an implicit reference (a zero-arity `super` or a `binding` call) counts as a use of the argument before it is shadowed. |

## Blind spots

None recorded.

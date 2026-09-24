# Style/GuardClause

Checks for conditionals that can be replaced with guard clauses.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

A condition with an `elsif` or `else` branch is allowed unless one of
`return`, `break`, `next`, `raise`, or `fail` is used in the body of the
conditional expression.

```ruby
# bad
def test
  if something
    work
  end
end

# good
def test
  return unless something

  work
end

# also good
def test
  work if something
end

# bad
if something
  raise 'exception'
else
  ok
end

# good
raise 'exception' if something
ok

# bad
define_method(:test) do
  if something
    work
  end
end

# good
define_method(:test) do
  return unless something

  work
end

# also good
define_method(:test) do
  work if something
end
```

With `AllowConsecutiveConditionals: true`, an ending `if`/`unless` directly
preceded by another `if`/`unless` statement (with no intervening code) is
not flagged.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| MinBodyLength | 1 |  | The number of lines a conditional's body needs to trigger this cop. |
| AllowConsecutiveConditionals | false |  | Allow an ending `if`/`unless` directly preceded by another one. |

## Blind spots

`node.parent&.assignment?` (RuboCop skips an `if`/`unless` used as the value
of an assignment) is approximated by tracking every simple and compound
assignment kind (`=`, `+=`, `||=`, `&&=`) over local/instance/class/global
variables, constants, constant paths, attribute writers, index writers, and
multiple assignment.
`node.method?(:define_method)` does not check the call's receiver, matching
RuboCop, so `obj.define_method(...) do ... end` is treated the same as a
bare call.

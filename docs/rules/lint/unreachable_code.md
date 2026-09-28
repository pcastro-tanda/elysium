# Lint/UnreachableCode

Checks for unreachable code.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

The check are based on the presence of flow of control
statement in non-final position in `begin` (implicit) blocks.

```ruby
# bad
def some_method
  return
  do_something
end

# bad
def some_method
  if cond
    return
  else
    return
  end
  do_something
end

# good
def some_method
  do_something
end
```

## Options

This rule has no options.

## Blind spots

`retry` outside a `rescue`/`begin` clause -- upstream's own fixture for it
is skipped under a Prism-backed parser, since Prism (unlike `parser`) treats
a top-level `retry` as a syntax error rather than a valid (if pointless)
keyword, so no such source can ever reach this rule to exercise the branch.
`@redefined`/`@instance_eval_count`-based suppression is otherwise exact:
a `def`/`def self.` for one of `raise`/`fail`/`throw`/`exit`/`exit!`/
`abort` only counts when it sits somewhere the `flow_expression?` recursion
actually reaches (a statement checked directly, or nested in a `begin`/
`if`/`unless`/`case`/`case`-`in` branch it recurses through) -- a
redefinition nested inside another `def`'s own body, a block, or a class/
module body is never seen, matching upstream's identical blind spot.

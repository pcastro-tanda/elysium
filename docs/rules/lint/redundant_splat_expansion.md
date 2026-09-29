# Lint/RedundantSplatExpansion

Checks for splat unnecessarily being called on literals.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for unneeded usages of splat expansion.

```ruby
# bad
a = *[1, 2, 3]
['a', 'b', *%w(c d e), 'f', 'g']

# good
c = [1, 2, 3]
a = *c
a = *1..10

# bad
do_something(*['foo', 'bar', 'baz'])

# good
do_something('foo', 'bar', 'baz')

# bad
case foo
when *[1, 2, 3]
  bar
end

# good
case foo
when 1, 2, 3
  bar
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowPercentLiteralArrayArgument | true |  | Allows a percent literal array being used as a method argument (`do_something(*%w[foo bar baz])`). |

## Blind spots

None recorded.

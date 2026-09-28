# Lint/EachWithObjectArgument

Checks if `each_with_object` is called with an immutable argument.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks if each_with_object is called with an immutable
argument. Since the argument is the object that the given block shall
make calls on to build something based on the enumerable that
each_with_object iterates over, an immutable argument makes no sense.
It's definitely a bug.

```ruby
# bad
sum = numbers.each_with_object(0) { |e, a| a += e }

# good
num = 0
sum = numbers.each_with_object(num) { |e, a| a += e }
```

## Options

This rule has no options.

## Blind spots

Only literal argument expressions are recognized (rubocop-ast's
`IMMUTABLE_LITERALS`: integers, floats, symbols, `true`/`false`/`nil`,
complex and rational literals). A variable or method call that is known at
runtime to hold one of these values, or a frozen mutable literal (e.g.
`''.freeze`), is not flagged -- matching upstream, which never traces
values back to their definitions.

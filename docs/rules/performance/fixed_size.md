# Performance/FixedSize

Do not compute the size of statically sized objects except in constants.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Do not compute the size of statically sized objects.

```ruby
# bad
'foo'.size
%q[bar].count
:fred.size
[1, 2, thud].count
{ a: corge, b: grault }.length

# good
foo.size
:"#{fred}".size
CONST = :baz.length
[1, 2, *thud].count
{ a: corge, **grault }.length
```

## Options

This rule has no options.

## Blind spots

None recorded.

# Lint/UselessTimes

Checks for useless `Integer#times` calls.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for uses of `Integer#times` that will never yield
(when the integer `<= 0`) or that will only ever yield once
(`1.times`).

# Safety

This cop is unsafe as `times` returns its receiver, which is
*usually* OK, but might change behavior.

```ruby
# bad
-5.times { do_something }
0.times { do_something }
1.times { do_something  }
1.times { |i| do_something(i) }

# good
do_something
do_something(1)
```

## Options

This rule has no options.

## Blind spots

None recorded.

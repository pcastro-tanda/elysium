# Lint/ItWithoutArgumentsInBlock

Checks uses of `it` calls without arguments in block.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

`it` calls without arguments will refer to the first block param in Ruby 3.4.
So use `it()` or `self.it` to ensure compatibility.

```ruby
# bad
do_something { it }

# good
do_something { it() }
do_something { self.it }
```

## Options

This rule has no options.

## Blind spots

None recorded.

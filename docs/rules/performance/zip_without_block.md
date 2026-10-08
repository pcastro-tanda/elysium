# Performance/ZipWithoutBlock

Checks for `map { |id| [id] }` and suggests replacing it with `zip`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for `map { |id| [id] }` and suggests replacing it with `zip`.

This cop is unsafe for novel definitions of `map` and `collect` on non-Enumerable objects that do not respond to `zip`.

```ruby
# bad
[1, 2, 3].map { |id| [id] }

# good
[1, 2, 3].zip
```

## Options

This rule has no options.

## Blind spots

None recorded.

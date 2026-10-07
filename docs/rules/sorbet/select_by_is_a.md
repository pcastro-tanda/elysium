# Sorbet/SelectByIsA

Suggests using `grep` over `select` when using it only for type narrowing. This is because Sorbet can properly infer types when using `grep` but not with `select`.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Suggests using `grep` over `select` when using it only for type narrowing.

```ruby
# bad
strings_or_integers.select { |e| e.is_a?(String) }
strings_or_integers.filter { |e| e.is_a?(String) }
strings_or_integers.select { |e| e.kind_of?(String) }

# good
strings_or_integers.grep(String)
```

## Options

This rule has no options.

## Blind spots

None recorded.

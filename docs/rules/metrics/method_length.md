# Metrics/MethodLength

Avoid methods longer than 10 lines of code.

| | |
| --- | --- |
| Department | Metrics |
| Enabled by default | true |
| Default severity | refactor |
| Fix | none |
| Stability | stable |

Checks if the length of a method exceeds some maximum value. Comment lines can optionally be allowed with `CountComments`. Constructs listed in `CountAsOne` (`array`, `hash`, `heredoc`, `method_call`) each collapse to a single counted line regardless of their own size. A `define_method` block is measured the same way as an ordinary method definition.

```ruby
# bad
Max: 2
def m
  a = 1
  a = 2
  a = 3
end

# good
Max: 2
def m
  a = 1
  a = 2
end
```

`AllowedMethods`/`AllowedPatterns` (both default to `[]`) exempt a method, or a `define_method` block whose first argument is a literal `Symbol`/`String` naming it, by name.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Max | 10 |  | Maximum number of counted lines a method may have. |
| CountComments | false |  | Whether full-line comments count towards the total. |
| CountAsOne | `[]` | `array`, `hash`, `heredoc`, `method_call` | Constructs that count as a single line regardless of their own size. |
| AllowedMethods | `[]` |  | Method names that are never measured. The deprecated `IgnoredMethods`/`ExcludedMethods` aliases are merged in too. |
| AllowedPatterns | `[]` |  | Method name regex patterns that are never measured. |

## Blind spots

`AllowedPatterns` entries that fail to compile as a Rust regex are dropped (never match) rather than raising a configuration error.

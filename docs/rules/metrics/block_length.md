# Metrics/BlockLength

Avoid long blocks with many lines.

| | |
| --- | --- |
| Department | Metrics |
| Enabled by default | true |
| Default severity | refactor |
| Fix | none |
| Stability | stable |

Checks if the length of a block exceeds some maximum value. Comment lines can optionally be ignored with `CountComments`. Constructs listed in `CountAsOne` (`array`, `hash`, `heredoc`, `method_call`) each collapse to a single counted line regardless of their own size. This cop does not apply to `Struct.new`/`Class.new`/`Module.new`/`Data.define` blocks.

```ruby
# bad
Max: 2
something do
  a = 1
  a = 2
  a = 3
end

# good
Max: 2
something do
  a = 1
  a = 2
end
```

`AllowedMethods` (default: `[refine]`) exempts a block by its owning call's method name; an entry containing a literal `.` (e.g. `Foo.bar`) additionally requires the call's receiver source (whitespace stripped) to equal the part before the `.`. `AllowedPatterns` matches the same method name against a list of regexps.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Max | 25 |  | Maximum number of counted lines a block may have. |
| CountComments | false |  | Whether full-line comments count towards the total. |
| CountAsOne | `[]` | `array`, `hash`, `heredoc`, `method_call` | Constructs that count as a single line regardless of their own size. |
| AllowedMethods | `refine` |  | Method (optionally `receiver.method`) names owning a block that is never measured. The deprecated `IgnoredMethods`/`ExcludedMethods` aliases are merged in too. |
| AllowedPatterns | `[]` |  | Method name regex patterns owning a block that is never measured. |

## Blind spots

`AllowedPatterns` entries that fail to compile as a Rust regex are dropped (never match) rather than raising a configuration error.

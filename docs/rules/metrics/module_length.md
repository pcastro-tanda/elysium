# Metrics/ModuleLength

Avoid modules longer than 100 lines of code.

| | |
| --- | --- |
| Department | Metrics |
| Enabled by default | true |
| Default severity | refactor |
| Fix | none |
| Stability | stable |

Checks if the length of a module exceeds some maximum value. Comment lines can optionally be ignored with `CountComments`. Constructs listed in `CountAsOne` (`array`, `hash`, `heredoc`, `method_call`) each collapse to a single counted line regardless of their own size. This also applies to a plain `FOO = Module.new do ... end` constant assignment (reported at the constant name, not the `Module.new` value).

```ruby
# bad
Max: 4
module Foo
  def a; end
  def b; end
  def c; end
  def d; end
  def e; end
end

# good
Max: 4
module Foo
  def a; end
  def b; end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Max | 100 |  | Maximum number of counted lines a module may have. |
| CountComments | false |  | Whether full-line comments count towards the total. |
| CountAsOne | `[]` | `array`, `hash`, `heredoc`, `method_call` | Constructs that count as a single line regardless of their own size. |

## Blind spots

`FOO ||= Module.new do ... end`/`FOO &&= ...`/`FOO op= ...` and multiple assignment (`FOO, BAR = Module.new do ... end`) are never measured, matching a real upstream gap (`module_definition?`'s node pattern only ever matches a plain `FOO = Module.new do ... end`), not a porting shortcut.

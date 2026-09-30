# Metrics/ClassLength

Avoid classes longer than 100 lines of code.

| | |
| --- | --- |
| Department | Metrics |
| Enabled by default | true |
| Default severity | refactor |
| Fix | none |
| Stability | stable |

Checks if the length of a class exceeds some maximum value. Comment lines can optionally be ignored with `CountComments`. Constructs listed in `CountAsOne` (`array`, `hash`, `heredoc`, `method_call`) each collapse to a single counted line regardless of their own size. This also applies to `Struct.new`/`Class.new` definitions, including ones assigned to a constant, and to a top-level `class << self`.

```ruby
# bad
Max: 4
class Foo
  def a; end
  def b; end
  def c; end
  def d; end
  def e; end
end

# good
Max: 4
class Foo
  def a; end
  def b; end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Max | 100 |  | Maximum number of counted lines a class may have. |
| CountComments | false |  | Whether full-line comments count towards the total. |
| CountAsOne | `[]` | `array`, `hash`, `heredoc`, `method_call` | Constructs that count as a single line regardless of their own size. |

## Blind spots

None recorded.

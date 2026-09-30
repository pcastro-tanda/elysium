# Layout/DotPosition

Checks the position of the dot in multi-line method calls.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks the `.` position in multi-line method calls.

```ruby
# EnforcedStyle: leading (default)

# bad
something.
  method

# good
something
  .method
```

```ruby
# EnforcedStyle: trailing

# bad
something
  .method

# good
something.
  method
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `leading` | `leading`, `trailing` | Whether the dot connecting a multi-line method call to its receiver belongs on the next line with the method name (`leading`) or the previous line with the receiver (`trailing`). |

## Blind spots

`self.autocorrect_incompatible_with` (`Style::RedundantSelf`, avoiding a
double-correction clash when both cops run together) is not ported: this
port has no cross-rule autocorrect-conflict mechanism.

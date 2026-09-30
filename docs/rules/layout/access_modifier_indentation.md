# Layout/AccessModifierIndentation

Checks indentation of private/protected visibility modifiers.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Bare access modifiers (those not applying to specific methods) should be
indented as deep as method definitions, or as deep as the `class`/`module`
keyword, depending on configuration.

```ruby
# EnforcedStyle: indent (default)

# bad
class Plumbus
private
  def smooth; end
end

# good
class Plumbus
  private
  def smooth; end
end
```

```ruby
# EnforcedStyle: outdent

# bad
class Plumbus
  private
  def smooth; end
end

# good
class Plumbus
private
  def smooth; end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `indent` | `outdent`, `indent` | The indentation style used to align access modifiers. |
| IndentationWidth | `nil` |  | Number of spaces to use for indentation. Defaults to `Layout/IndentationWidth`'s `Width`. |

## Blind spots

RuboCop's `ConfigurableEnforcedStyle` auto-style-detection bookkeeping (`correct_style_detected`/`opposite_style_detected`/`unrecognized_style_detected`, used only by `rubocop --auto-gen-config`) is not replicated; it never changes whether an offense is reported or how it is fixed.

# Style/Lambda

Use the new lambda literal syntax for single-line blocks.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks the usage of a `lambda` literal syntax for single-line blocks and
method calls for multiline blocks. It is configurable to enforce one of the
styles for both single line and multiline lambdas as well.

```ruby
# EnforcedStyle: line_count_dependent (default)
# bad
f = lambda { |x| x }
f = ->(x) do
      x
    end

# good
f = ->(x) { x }
f = lambda do |x|
      x
    end
```

```ruby
# EnforcedStyle: lambda
# bad
f = ->(x) { x }
f = ->(x) do
      x
    end

# good
f = lambda { |x| x }
f = lambda do |x|
      x
    end
```

```ruby
# EnforcedStyle: literal
# bad
f = lambda { |x| x }
f = lambda do |x|
      x
    end

# good
f = ->(x) { x }
f = ->(x) do
      x
    end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `line_count_dependent` | `line_count_dependent`, `lambda`, `literal` | Which lambda syntax to enforce. |

## Blind spots

`self.autocorrect_incompatible_with` (`Style::SymbolProc`, avoiding a
`->(x)(&:method)` double-correction clash when both cops run together) is
not ported: this port has no cross-rule autocorrect-conflict mechanism.

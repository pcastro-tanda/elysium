# Layout/SpaceAroundBlockParameters

Checks the spacing inside and after block parameters pipes.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks the spacing inside and after block parameters pipes. Line breaks
inside parameter pipes are checked by `Layout/MultilineBlockLayout` and
not by this cop. Spaces inside a lambda's parameter parentheses are left
to `Layout/SpaceInsideParens` when that cop enforces a conflicting style.

```ruby
# EnforcedStyleInsidePipes: no_space (default)

# bad
{}.each { | x,  y |puts x }
->( x,  y ) { puts x }

# good
{}.each { |x, y| puts x }
->(x, y) { puts x }
```

```ruby
# EnforcedStyleInsidePipes: space

# bad
{}.each { |x,  y| puts x }
->(x,  y) { puts x }

# good
{}.each { | x, y | puts x }
->( x, y ) { puts x }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyleInsidePipes | `no_space` | `space`, `no_space` | Whether a single space is required (`space`) or forbidden (`no_space`) right inside the parameter pipes/parens. |

## Blind spots

None recorded.

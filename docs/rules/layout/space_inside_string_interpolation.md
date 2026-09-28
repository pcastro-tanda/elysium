# Layout/SpaceInsideStringInterpolation

Checks for whitespace within string interpolations.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: no_space (default)

# bad
var = "This is the #{ space } example"

# good
var = "This is the #{no_space} example"
```

```ruby
# EnforcedStyle: space

# bad
var = "This is the #{no_space} example"

# good
var = "This is the #{ space } example"
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `no_space` | `space`, `no_space` | Whether string interpolation requires or forbids surrounding space. |

## Blind spots

Reproduces RuboCop's lexer-token-based `empty_brackets?`/`extra_space?` by
scanning raw bytes between the `#{`/`}` delimiter spans rather than walking a
token stream (see the module docs for the exact mapping); this reaches every
case the cop's own logic reaches, but treats `[ \t]` runs the same way
upstream's `SINGLE_SPACE_REGEXP` does, so a lone vertical tab or form feed
directly abutting a delimiter is not recognized as space (an extremely
unlikely real-world input, matching an existing documented deviation in
`Layout/SpaceInsideHashLiteralBraces`).

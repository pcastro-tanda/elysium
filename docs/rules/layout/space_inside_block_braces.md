# Layout/SpaceInsideBlockBraces

Checks that block braces have or don't have surrounding space.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

For blocks taking parameters, checks that the left brace has or doesn't have
trailing space depending on configuration.

```ruby
# bad (EnforcedStyle: space, the default)
some_array.each {puts e}

# good (EnforcedStyle: space, the default)
some_array.each { puts e }

# bad (EnforcedStyle: no_space)
some_array.each { puts e }

# good (EnforcedStyle: no_space)
some_array.each {puts e}

# bad (EnforcedStyleForEmptyBraces: no_space, the default)
some_array.each { }

# good (EnforcedStyleForEmptyBraces: no_space, the default)
some_array.each {}

# bad (EnforcedStyleForEmptyBraces: space)
some_array.each {}

# good (EnforcedStyleForEmptyBraces: space)
some_array.each { }

# bad (SpaceBeforeBlockParameters: true, the default)
[1, 2, 3].each {|n| n * 2 }

# good (SpaceBeforeBlockParameters: true, the default)
[1, 2, 3].each { |n| n * 2 }

# bad (SpaceBeforeBlockParameters: false)
[1, 2, 3].each { |n| n * 2 }

# good (SpaceBeforeBlockParameters: false)
[1, 2, 3].each {|n| n * 2 }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `space` | `space`, `no_space` | Whether block braces have surrounding space. |
| EnforcedStyleForEmptyBraces | `no_space` | `space`, `no_space` | Whether empty block braces have a space in between. |
| SpaceBeforeBlockParameters | true |  | Whether there is a space between `{` and `|`. Overrides `EnforcedStyle` if there is a conflict. |

## Blind spots

`do`/`end` blocks are always skipped (matches RuboCop's `node.keywords?` guard). Ruby's `/\R/` (any Unicode line separator) is approximated as a bare `\n` check, and `/\s/` as ASCII whitespace; both cover every case Ruby source can produce here except literal Unicode line separators inside a block's braces, which do not occur in practice.

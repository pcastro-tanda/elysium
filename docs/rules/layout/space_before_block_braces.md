# Layout/SpaceBeforeBlockBraces

Checks that the left block brace has or doesn't have space before it.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that block braces have or don't have a space before the opening
brace depending on configuration.

```ruby
# bad (EnforcedStyle: space, the default)
foo.map{ |a|
  a.bar.to_s
}

# good (EnforcedStyle: space, the default)
foo.map { |a|
  a.bar.to_s
}

# bad (EnforcedStyle: no_space)
foo.map { |a|
  a.bar.to_s
}

# good (EnforcedStyle: no_space)
foo.map{ |a|
  a.bar.to_s
}

# bad (EnforcedStyleForEmptyBraces: space, the default)
7.times{}

# good (EnforcedStyleForEmptyBraces: space, the default)
7.times {}

# bad (EnforcedStyleForEmptyBraces: no_space)
7.times {}

# good (EnforcedStyleForEmptyBraces: no_space)
7.times{}
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `space` | `space`, `no_space` | Whether the left block brace has space before it. |
| EnforcedStyleForEmptyBraces | `space` | `space`, `no_space` | Whether empty braces have space before them. |

## Blind spots

`self.autocorrect_incompatible_with` (`Style::SymbolProc`) is not
replicated; it only matters when both cops run in the same fix pass.
`config_to_allow_offenses`/`handle_different_styles_for_empty_braces`
auto-config generation (the `--auto-gen-config` result when mixed styles
are used) is not ported: this engine has no config-generation pass.

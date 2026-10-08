# Sorbet/ValidGemVersionAnnotations

Checks that gem versions in RBI annotations are properly formatted per the Bundler gem specification.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks that gem versions in RBI annotations are properly formatted per the Bundler gem specification.

```ruby
# bad
# @version > not a version number

# good
# @version = 1

# good
# @version > 1.2.3

# good
# @version <= 4.3-preview
```

## Options

This rule has no options.

## Blind spots

None recorded.

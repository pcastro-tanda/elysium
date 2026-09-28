# Style/MixinUsage

Checks that `include`, `extend` and `prepend` statements appear inside classes and modules, not at the top level.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks that `include`, `extend` and `prepend` statements appear
inside classes and modules, not at the top level, so as to not affect
the behavior of `Object`.

```ruby
# bad
include M

class C
end

# bad
extend M

class C
end

# bad
prepend M

class C
end

# good
class C
  include M
end

# good
class C
  extend M
end

# good
class C
  prepend M
end
```

## Options

This rule has no options.

## Blind spots

Only a single constant argument is recognized (`include M`, `include M1::M2::M3`), matching
upstream's fixed-arity `(send nil? {:include :extend :prepend} const)` pattern: `include M1, M2`
(two mixins in one call) never matches upstream's pattern either, and is not flagged here.

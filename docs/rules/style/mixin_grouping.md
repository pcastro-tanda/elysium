# Style/MixinGrouping

Checks for grouping of mixins in `class` and `module` bodies.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

By default it enforces mixins to be placed in separate declarations, but it
can be configured to enforce grouping them in one declaration.

```ruby
# EnforcedStyle: separated (default)
# bad
class Foo
  include Bar, Qox
end

# good
class Foo
  include Qox
  include Bar
end
```

```ruby
# EnforcedStyle: grouped
# bad
class Foo
  extend Bar
  extend Qox
end

# good
class Foo
  extend Qox, Bar
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `separated` | `separated`, `grouped` | Whether mixins of the same kind must be spread across separate statements, or merged into a single statement. |

## Blind spots

None recorded.

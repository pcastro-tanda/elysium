# Layout/EmptyLinesAroundModuleBody

Keeps track of empty lines around module bodies.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: no_empty_lines (default)

# bad
module Foo

  def bar
    # ...
  end

end

# good
module Foo
  def bar
    # ...
  end
end
```

```ruby
# EnforcedStyle: empty_lines

# bad
module Foo
  def bar
    # ...
  end
end

# good
module Foo

  def bar
    # ...
  end

end
```

```ruby
# EnforcedStyle: empty_lines_except_namespace

# good
module Foo
  module Bar

    # ...

  end
end
```

```ruby
# EnforcedStyle: empty_lines_special

# good
module Foo

  def bar; end

end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `no_empty_lines` | `empty_lines`, `empty_lines_except_namespace`, `empty_lines_special`, `no_empty_lines` | The blank-line convention required around a module body. |

## Blind spots

None recorded.

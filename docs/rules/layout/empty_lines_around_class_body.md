# Layout/EmptyLinesAroundClassBody

Keeps track of empty lines around class bodies.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: no_empty_lines (default)

# good
class Foo
  def bar
    # ...
  end
end
```

```ruby
# EnforcedStyle: empty_lines

# good
class Foo

  def bar
    # ...
  end

end
```

```ruby
# EnforcedStyle: empty_lines_except_namespace

# good
class Foo
  class Bar

    # ...

  end
end
```

```ruby
# EnforcedStyle: empty_lines_special

# good
class Foo

  def bar; end

end
```

```ruby
# EnforcedStyle: beginning_only

# good
class Foo

  def bar
    # ...
  end
end
```

```ruby
# EnforcedStyle: ending_only

# good
class Foo
  def bar
    # ...
  end

end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `no_empty_lines` | `empty_lines`, `empty_lines_except_namespace`, `empty_lines_special`, `no_empty_lines`, `beginning_only`, `ending_only` | The blank-line convention required around a class body. |

## Blind spots

`comment_line?` (used by the `empty_lines_special` deferred-blank-line
check to skip back over full-line comments) approximates RuboCop's
`/^\s*#/` with a plain space/tab scan, not full Ruby `\s` (which also
matches form feed and vertical tab); this only differs on lines using those
control characters as indentation, which does not occur in practice.

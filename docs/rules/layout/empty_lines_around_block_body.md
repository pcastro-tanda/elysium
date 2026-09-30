# Layout/EmptyLinesAroundBlockBody

Keeps track of empty lines around block bodies.

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
foo do |bar|

  # ...

end

# good
foo do |bar|
  # ...
end
```

```ruby
# EnforcedStyle: empty_lines

# bad
foo do |bar|
  # ...
end

# good
foo do |bar|

  # ...

end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `no_empty_lines` | `empty_lines`, `no_empty_lines` | The blank-line convention required around a block body. |

## Blind spots

None recorded.

# Layout/LineContinuationSpacing

Checks the spacing in front of backslash in line continuations.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: space (default)

# bad
'a'\
'b'  \
'c'

# good
'a' \
'b' \
'c'
```

```ruby
# EnforcedStyle: no_space

# bad
'a' \
'b'  \
'c'

# good
'a'\
'b'\
'c'
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `space` | `space`, `no_space` | The spacing style to enforce in front of a line-continuation backslash. |

## Blind spots

None recorded.

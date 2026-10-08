# Layout/FirstHashElementLineBreak

Checks for a line break before the first element in a multi-line hash.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
{ a: 1,
  b: 2}

# good
{
  a: 1,
  b: 2 }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowMultilineFinalElement | false |  | Whether the last element of the hash is allowed to start a new, multi-line element without triggering this cop. |

## Blind spots

None recorded.

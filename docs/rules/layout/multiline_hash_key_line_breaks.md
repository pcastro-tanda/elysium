# Layout/MultilineHashKeyLineBreaks

Checks that each item in a multi-line hash literal starts on a separate line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
{
  a: 1, b: 2,
  c: 3
}

# good
{
  a: 1,
  b: 2,
  c: 3
}
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowMultilineFinalElement | false |  | Whether the last key in the hash is allowed to start a new, multi-line element without triggering this cop. |

## Blind spots

None recorded.

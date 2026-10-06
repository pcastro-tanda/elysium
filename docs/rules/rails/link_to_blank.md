# Rails/LinkToBlank

Checks that `link_to` with a `target: "_blank"` have a `rel: "noopener"` option passed to them.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that `link_to` (and `link_to_if` / `link_to_unless`) with a `target: "_blank"` have a `rel: "noopener"` option passed to them.

```ruby
# bad
link_to "Click here", "https://www.example.com", target: "_blank"

# good
link_to "Click here", "https://www.example.com", target: "_blank", rel: "noopener"
```

## Options

This rule has no options.

## Blind spots

None recorded.

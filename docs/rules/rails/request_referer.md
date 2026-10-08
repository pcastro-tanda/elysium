# Rails/RequestReferer

Use consistent syntax for request.referer.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for consistent uses of `request.referer` or
`request.referrer`, depending on the cop's configuration.

```ruby
# EnforcedStyle: referer (default)
# bad
request.referrer

# good
request.referer

# EnforcedStyle: referrer
# bad
request.referer

# good
request.referrer
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `referer` | `referer`, `referrer` | Which spelling of `request.referer` is enforced. |

## Blind spots

None recorded.

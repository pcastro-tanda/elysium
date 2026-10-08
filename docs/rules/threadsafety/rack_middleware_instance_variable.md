# ThreadSafety/RackMiddlewareInstanceVariable

Avoid instance variables in Rack middleware.

| | |
| --- | --- |
| Department | ThreadSafety |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Middlewares are initialized once, meaning any instance variables are shared between executor threads. To avoid potential race conditions, design middlewares to be stateless or implement proper synchronization.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedIdentifiers | `[]` |  | Instance variable names (sigils stripped) that are never reported. |

## Blind spots

None recorded.

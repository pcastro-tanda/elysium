# Rails/HttpStatus

Enforces use of symbolic or numeric value to define HTTP status.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces use of symbolic or numeric value to define HTTP status.

```ruby
# EnforcedStyle: symbolic (default)
# bad
render :foo, status: 200
head 200

# good
render :foo, status: :ok
head :ok

# EnforcedStyle: numeric
# bad
render :foo, status: :ok
head :ok

# good
render :foo, status: 200
head 200
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `symbolic` | `numeric`, `symbolic` | Whether to prefer symbolic or numeric HTTP statuses. |

## Blind spots

The status table is `Rack::Utils::SYMBOL_TO_STATUS_CODE` of rack 3.2.7; other rack versions differ for a few statuses (e.g. `:unprocessable_entity`).

# Rails/HttpStatusNameConsistency

Enforces consistency by using the current HTTP status names.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Enforces consistency by using the current HTTP status names.

```ruby
# bad
render json: { error: "Invalid data" }, status: :unprocessable_entity
head :payload_too_large

# good
render json: { error: "Invalid data" }, status: :unprocessable_content
head :content_too_large
```

## Options

This rule has no options.

## Blind spots

None recorded.

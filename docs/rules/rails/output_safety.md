# Rails/OutputSafety

The use of `html_safe` or `raw` may be a security risk.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for the use of output safety calls like `html_safe`, `raw`, and `safe_concat`. These methods do not escape content. They simply return a SafeBuffer containing the content as is. Instead, use `safe_join` to join content and escape it and concat to concatenate content and escape it, ensuring its safety.

```ruby
# bad
user_content = "<b>hi</b>"
safe = user_content.html_safe
raw(user_content)

# good
safe_join(["<b>".html_safe, user_content, "</b>".html_safe])
```

## Options

This rule has no options.

## Blind spots

None recorded.

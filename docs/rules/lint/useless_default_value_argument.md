# Lint/UselessDefaultValueArgument

Checks for usage of `fetch` or `Array.new` with default value argument and block.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for usage of method `fetch` or `Array.new` with default value argument and block. In such cases, block will always be used as default value.

This cop emulates Ruby warning "block supersedes default value argument" which applies to `Array.new`, `Array#fetch`, `Hash#fetch`, `ENV.fetch` and `Thread#fetch`.

A `fetch` call without a receiver is considered a custom method and does not register an offense.

```ruby
# bad
x.fetch(key, default_value) { block_value }
Array.new(size, default_value) { block_value }

# good
x.fetch(key) { block_value }
Array.new(size) { block_value }

# also good - in case default value argument is desired instead
x.fetch(key, default_value)
Array.new(size, default_value)

# good - keyword arguments aren't registered as offenses
x.fetch(key, keyword: :arg) { block_value }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedReceivers | `[]` |  | Receiver names (`AllowedReceivers#receiver_name`) never flagged. |

## Blind spots

None recorded.

# Performance/StringBytesize

Use `String#bytesize` instead of calculating the size of the bytes array.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for calls to `#bytes` counting method and suggests using `bytesize` instead. The `bytesize` method is more efficient and directly returns the size in bytes, avoiding the intermediate array allocation that `bytes.size` incurs.

This cop is unsafe because it assumes that the receiver responds to `#bytesize` method.

```ruby
# bad
string_var.bytes.count
"foobar".bytes.size

# good
string_var.bytesize
"foobar".bytesize
```

## Options

This rule has no options.

## Blind spots

None recorded.

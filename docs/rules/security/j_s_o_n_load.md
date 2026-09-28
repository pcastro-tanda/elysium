# Security/JSONLoad

Prefer usage of `JSON.parse` over `JSON.load` due to potential security issues. See reference for more information.

| | |
| --- | --- |
| Department | Security |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for the use of JSON class methods which have potential
security issues.

`JSON.load` and similar methods allow deserialization of arbitrary ruby objects:

```ruby
require 'json/add/string'
result = JSON.load('{ "json_class": "String", "raw": [72, 101, 108, 108, 111] }')
pp result # => "Hello"
```

Never use `JSON.load` for untrusted user input. Prefer `JSON.parse` unless you have
a concrete use-case for `JSON.load`.

```ruby
# bad
JSON.load('{}')
JSON.restore('{}')

# good
JSON.parse('{}')
JSON.unsafe_load('{}')

# good - explicit use of `create_additions` option
JSON.load('{}', create_additions: true)
JSON.load('{}', create_additions: false)
```

## Options

This rule has no options.

## Blind spots

None recorded.

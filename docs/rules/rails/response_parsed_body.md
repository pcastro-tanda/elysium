# Rails/ResponseParsedBody

Prefer `response.parsed_body` to custom parsing logic for `response.body`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Prefer `response.parsed_body` to custom parsing logic for `response.body`.

This cop's autocorrection is unsafe because it assumes the response content type matches the parser used.

```ruby
# bad
JSON.parse(response.body)

# good
response.parsed_body
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.

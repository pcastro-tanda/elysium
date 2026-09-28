# Lint/ToJSON

Ensure #to_json includes an optional argument.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks to make sure `#to_json` includes an optional argument. When overriding `#to_json`, callers may invoke JSON generation via `JSON.generate(your_obj)`. Since `JSON#generate` allows for an optional argument, your method should too.

## Options

This rule has no options.

## Blind spots

None recorded.

# Style/Attr

Checks for uses of Module#attr.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of `Module#attr`. The `attr` method has confusing behavior: with a single argument it creates a reader (like `attr_reader`), but with a second boolean argument it creates an accessor (deprecated in Ruby 1.9). Use `attr_reader` or `attr_accessor` to make intent explicit.

## Options

This rule has no options.

## Blind spots

None recorded.

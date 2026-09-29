# Style/ExpandPathArguments

Use `expand_path(__dir__)` instead of `expand_path('..', __FILE__)`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for use of the `File.expand_path` arguments.
Likewise, it also checks for the `Pathname.new` argument.

```ruby
# bad
File.expand_path('..', __FILE__)

# good
File.expand_path(__dir__)

# bad
Pathname.new(__FILE__).parent.expand_path

# good
Pathname.new(__dir__).expand_path
```

## Options

This rule has no options.

## Blind spots

None recorded.

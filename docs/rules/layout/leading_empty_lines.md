# Layout/LeadingEmptyLines

Checks for unnecessary blank lines at the beginning of a file.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
# (start of file)

class Foo
end

# bad
# (start of file)

# a comment

# good
# (start of file)
class Foo
end

# good
# (start of file)
# a comment
```

## Options

This rule has no options.

## Blind spots

None recorded.

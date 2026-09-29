# Style/PerlBackrefs

Avoid Perl-style regex back references.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Looks for uses of Perl-style regexp match backreferences and their English
versions like `$1`, `$2`, `$&`, `$MATCH`, `$PREMATCH`, etc.

```ruby
# bad
puts $1

# good
puts Regexp.last_match(1)
```

## Options

This rule has no options.

## Blind spots

None recorded.

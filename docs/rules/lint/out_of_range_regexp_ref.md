# Lint/OutOfRangeRegexpRef

Checks for out of range reference for Regexp because it always returns nil.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Looks for references of `Regexp` captures that are out of range
and thus always returns nil.

## Safety

This cop is unsafe because it is naive in how it determines what
references are available based on the last encountered regexp, but
it cannot handle some cases, such as conditional regexp matches, which
leads to false positives, such as:

```ruby
foo ? /(c)(b)/ =~ str : /(b)/ =~ str
do_something if $2
# $2 is defined for the first condition but not the second, however
# the cop will mark this as an offense.
```

This might be a good indication of code that should be refactored,
however.

```ruby
/(foo)bar/ =~ 'foobar'

# bad - always returns nil

puts $2 # => nil

# good

puts $1 # => foo
```

## Options

This rule has no options.

## Blind spots

RuboCop's `regexp_parser`-based capture scan silently yields no captures at all for a pattern it fails to parse; this port's simpler scanner never fails to parse and always finds whatever named/numbered captures its grammar recognizes. No known fixture distinguishes the two.

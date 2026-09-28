# Lint/AmbiguousRegexpLiteral

Checks for ambiguous regexp literals in the first argument of a method invocation without parentheses.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

```ruby
# bad

# This is interpreted as a method invocation with a regexp literal,
# but it could possibly be `/` method invocations.
# (i.e. `do_something./(pattern)./(i)`)
do_something /pattern/i

# good

# With parentheses, there's no ambiguity.
do_something(/pattern/i)
```

## Options

This rule has no options.

## Blind spots

Upstream branches on `target_ruby_version >= 3.0` to also recognize the pre-3.0 `:ambiguous_literal`
diagnostic reason; Prism only ever parses as 3.3+, so that branch (and the `TargetRubyVersion`
option it depends on) is dead here and not read.

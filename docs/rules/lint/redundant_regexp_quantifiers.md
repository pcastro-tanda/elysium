# Lint/RedundantRegexpQuantifiers

Checks for redundant quantifiers in Regexps.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

It is always allowed when interpolation is used in a regexp literal, because it's unknown what kind of string will be expanded as a result:

```ruby
/(?:a*#{interpolation})?/x
```

```ruby
# bad
/(?:x+)+/

# good
/(?:x)+/

# good
/(?:x+)/

# bad
/(?:x+)?/

# good
/(?:x)*/

# good
/(?:x*)/
```

## Options

This rule has no options.

## Blind spots

RuboCop's `regexp_parser`-based tree fails to build for some malformed patterns, silently finding no redundant quantifiers at all in that case; this scanner never fails to parse. No known fixture distinguishes the two.

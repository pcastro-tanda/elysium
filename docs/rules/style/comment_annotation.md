# Style/CommentAnnotation

Checks formatting of special comments (TODO, FIXME, OPTIMIZE, HACK, REVIEW, NOTE).

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks that comment annotation keywords are written according to
guidelines.

Annotation keywords can be specified by overriding the cop's `Keywords`
configuration. Keywords are allowed to be single words or phrases.

NOTE: With a multiline comment block (where each line is only a comment),
only the first line will be able to register an offense, even if an
annotation keyword starts another line. This is done to prevent incorrect
registering of keywords (e.g. `review`) inside a paragraph as an
annotation.

```ruby
# RequireColon: true (default)

# bad
# TODO make better

# good
# TODO: make better

# bad
# TODO:make better

# good
# TODO: make better

# bad
# fixme: does not work

# good
# FIXME: does not work

# bad
# Optimize does not work

# good
# OPTIMIZE: does not work

# RequireColon: false

# bad
# TODO: make better

# good
# TODO make better

# bad
# fixme does not work

# good
# FIXME does not work

# bad
# Optimize does not work

# good
# OPTIMIZE does not work
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Keywords | `TODO`, `FIXME`, `OPTIMIZE`, `HACK`, `REVIEW`, `NOTE` |  | Annotation keywords to check for formatting. |
| RequireColon | true |  | Whether annotation keywords must be followed by a colon. |

## Blind spots

None recorded.

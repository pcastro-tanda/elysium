# Layout/TrailingEmptyLines

Checks trailing blank lines and final newline.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

A file should end with exactly one newline, with no extra blank lines before
it (or, under `EnforcedStyle: final_blank_line`, exactly one blank line
before the final newline).

```ruby
# EnforcedStyle: final_newline (default)

# bad
class Foo; end

# EOF

# bad
class Foo; end # EOF

# good
class Foo; end
# EOF
```

```ruby
# EnforcedStyle: final_blank_line

# bad
class Foo; end
# EOF

# bad
class Foo; end # EOF

# good
class Foo; end

# EOF
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `final_newline` | `final_newline`, `final_blank_line` | The blank-line convention required at the end of a file. |

## Blind spots

`__END__` detection is RuboCop's own heuristic: any occurrence of the literal
text `__END__` anywhere in the file (even inside a string or comment)
suppresses the cop, matching RuboCop's `buffer.source.match?(/\s*__END__/)`
check exactly (its token-based fallback is unreachable dead code in RuboCop
itself, since anything the fallback could find is already a substring of
`buffer.source` and so would already have matched the first check).

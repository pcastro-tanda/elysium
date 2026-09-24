# Style/RedundantRegexpEscape

Checks for redundant escapes inside `Regexp` literals.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
%r{foo\/bar}

# good
%r{foo/bar}

# good
/foo\/bar/

# good
%r/foo\/bar/

# good
%r!foo\!bar!

# bad
/a\-b/

# good
/a-b/

# bad
/[\+\-]\d/

# good
/[+\-]\d/
```

## Options

This rule has no options.

## Blind spots

`node.source[index]` in the original cop indexes the whole literal (opening delimiter included) with a content-relative offset, so its interpolation- sigil check is only correct for single-byte delimiters (`/.../`); we use "the content byte right before the backslash" instead, which is the evidently intended semantic and also covers multi-byte `%r` delimiters RuboCop itself mishandles. No known fixture exercises that discrepancy.

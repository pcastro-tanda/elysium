# Layout/EndOfLine

Checks for Windows-style line endings in the source code.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

```ruby
# EnforcedStyle: native (default)
# The `native` style means that CR+LF (Carriage Return + Line Feed) is
# enforced on Windows, and LF is enforced on other platforms.

# bad
puts 'Hello' # Return character is LF on Windows.
puts 'Hello' # Return character is CR+LF on other than Windows.

# good
puts 'Hello' # Return character is CR+LF on Windows.
puts 'Hello' # Return character is LF on other than Windows.
```

```ruby
# EnforcedStyle: lf
# The `lf` style means that LF (Line Feed) is enforced on all platforms.

# bad
puts 'Hello' # Return character is CR+LF on all platforms.

# good
puts 'Hello' # Return character is LF on all platforms.
```

```ruby
# EnforcedStyle: crlf
# The `crlf` style means that CR+LF (Carriage Return + Line Feed) is
# enforced on all platforms.

# bad
puts 'Hello' # Return character is LF on all platforms.

# good
puts 'Hello' # Return character is CR+LF on all platforms.
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `native` | `native`, `lf`, `crlf` | Which line-ending style to enforce. |

## Blind spots

`native` resolves once from the host OS the linter itself runs on
(`cfg!(windows)`), matching RuboCop's `Platform.windows?` check against the
machine running the cop -- not `TargetRubyVersion` or any other per-project
setting. Only the line containing the file's last real token is scanned in
RuboCop, everything after (including a `__END__` data section) is ignored;
this port approximates that boundary with the line before `__END__` when
present, or the file's last line otherwise, so a trailing blank or
comment-only tail with a stray inconsistent terminator (never seen in
practice) would not reproduce RuboCop's exact cutoff.

# Rails/StripHeredoc

Enforces the use of squiggly heredoc over strip_heredoc.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of squiggly heredoc over `strip_heredoc`.

```ruby
# bad
<<EOS.strip_heredoc
  some text
EOS

# bad
<<-EOS.strip_heredoc
  some text
EOS

# good
<<~EOS
  some text
EOS
```

## Options

This rule has no options.

## Blind spots

None recorded.

# Style/RedundantHeredocDelimiterQuotes

Checks for redundant heredoc delimiter quotes.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for redundant heredoc delimiter quotes.

```ruby
# bad
do_something(<<~'EOS')
  no string interpolation style text
EOS

# good
do_something(<<~EOS)
  no string interpolation style text
EOS

do_something(<<~'EOS')
  #{string_interpolation_style_text_not_evaluated}
EOS

do_something(<<~'EOS')
  Preserve \
  newlines
EOS
```

## Options

This rule has no options.

## Blind spots

None recorded.

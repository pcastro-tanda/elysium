# Layout/TrailingWhitespace

Looks for trailing whitespace in the source code.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

Trailing whitespace at the end of a line is invisible noise that shows up in
diffs. The fix deletes it.

```ruby
# bad (the line ends with a space)
x = 0 

# good
x = 0
```

Inside a heredoc, whitespace that is part of the string cannot simply be
deleted, so the fix wraps it in an interpolation instead:

```ruby
# bad
code = <<~RUBY
  x = 0 
RUBY

# good
code = <<~RUBY
  x = 0#{' '}
RUBY
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowInHeredoc | false |  | Allow trailing whitespace inside heredoc bodies. |

## Blind spots

Lines are read from the file as-is, so a `\r` before the line terminator is
treated as part of the line terminator rather than as trailing whitespace,
matching RuboCop's buffer handling.

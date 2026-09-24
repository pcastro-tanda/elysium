# Style/StringConcatenation

Checks for places where string concatenation can be replaced with string interpolation.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

```ruby
# bad
email_with_name = user.name + ' <' + user.email + '>'
Pathname.new('/') + 'test'

# good
email_with_name = "#{user.name} <#{user.email}>"
email_with_name = format('%s <%s>', user.name, user.email)
"#{Pathname.new('/')}test"

# accepted, line-end concatenation
name = 'First' +
  'Last'
```

With `Mode: conservative`, only a `+` whose left-hand side (the receiver) is a string literal is flagged; `Mode: aggressive` (the default) flags a `+` with a string literal on either side.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Mode | `aggressive` | `aggressive`, `conservative` | `aggressive` flags any `+` with a string literal on either side; `conservative` only flags a `+` whose receiver is a string literal. |

## Blind spots

- A multi-statement `#{a; b}` interpolation hole and a backtick `` ` `` (x)string part are not unwrapped for re-quoting the way a plain string or a single-expression interpolation hole is; both fall back to re-embedding their raw source, matching RuboCop's own `else` branch for any non-`str`/`dstr`/`begin` part.
- A right-associative chain built with explicit parentheses on the argument side (`a + (b + c)`) is vanishingly rare in practice and not verified against RuboCop's real `.parent`-climbing behavior beyond the `ArgumentsNode` wrapper Prism (unlike whitequark) interposes between a call and its arguments.

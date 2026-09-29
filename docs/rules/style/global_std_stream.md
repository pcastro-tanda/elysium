# Style/GlobalStdStream

Enforces the use of `$stdout/$stderr/$stdin` instead of `STDOUT/STDERR/STDIN`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

`STDOUT/STDERR/STDIN` are constants, and while you can actually reassign (possibly to redirect some stream) constants in Ruby, you'll get an interpreter warning if you do so. Additionally, `$stdout/$stderr/$stdin` can safely be accessed in a Ractor because they are ractor-local, while `STDOUT/STDERR/STDIN` will raise `Ractor::IsolationError`.

## Options

This rule has no options.

## Blind spots

None recorded.

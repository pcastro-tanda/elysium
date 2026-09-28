# Security/Open

Checks for the use of `Kernel#open` and `URI.open` with dynamic data.

| | |
| --- | --- |
| Department | Security |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

`Kernel#open` and `URI.open` enable not only file access but also process
invocation by prefixing a pipe symbol (e.g., `open("| ls")`).
So, it may lead to a serious security risk by using variable input to
the argument of `Kernel#open` and `URI.open`. It would be better to use
`File.open`, `IO.popen` or `URI.parse#open` explicitly.

NOTE: `open` and `URI.open` with literal strings are not flagged by this
cop.

```ruby
# bad
open(something)
open("| #{something}")
open("| foo")
URI.open(something)

# good
File.open(something)
IO.popen(something)
URI.parse(something).open

# good (literal strings)
open("foo.text")
URI.open("http://example.com")
URI.parse(url).open
```

## Options

This rule has no options.

## Blind spots

Upstream's own documented `@safety` caveat: this could register false
positives if `open` is redefined in a class and then used without a
receiver in that class. `safe?`'s `+`-concatenation branch only recurses
one level deep (the receiver must be a plain string literal, not another
concatenation or an interpolated string), matching upstream's own
`concatenated_string?` restriction exactly -- not a blind spot introduced
by this port.

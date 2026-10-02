# Style/FileOpen

Checks for `File.open` without a block, which can leak file descriptors.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

When `File.open` is called without a block, the caller is responsible
for closing the file descriptor. If it is not explicitly closed, it
will only be closed when the garbage collector runs, which may lead
to resource exhaustion. Using the block form ensures the file is
automatically closed when the block exits.

This cop only registers an offense when the result of `File.open` is
assigned to a variable or has a method chained on it, as those are the
clearest indicators that the block form should be used instead. When
`File.open` is used as a return value or passed as an argument, the
caller is likely managing the file descriptor intentionally.

@safety
This cop is unsafe because it relies on syntax heuristics and cannot
verify whether the file descriptor is safely managed. For example, it
still flags intentional one-shot reads (`File.open("f").read`) where
the file descriptor is closed by the garbage collector.

```ruby
# bad
f = File.open('file')

# bad
File.open('file').read

# good
File.open('file') do |f|
  f.read
end

# good
File.open('file', &:read)

# good - pass an open file object to an API that manages its lifecycle
process(io: File.open('file'))

# good - return an open file object for the caller to manage
def json_key_io
  File.open('file')
end

# good - use File.read for one-shot reads
File.read('file')
```

## Options

This rule has no options.

## Blind spots

None recorded.

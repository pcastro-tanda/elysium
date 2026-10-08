# Style/FileTouch

Favor `FileUtils.touch` for touching files.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for usage of `File.open` in append mode with empty block.

Such a usage only creates a new file, but it doesn't update
timestamps for an existing file, which might have been the intention.

For example, for an existing file `foo.txt`:

```
ruby -e "puts File.mtime('foo.txt')"
# 2024-11-26 12:17:23 +0100

ruby -e "File.open('foo.txt', 'a') {}"

ruby -e "puts File.mtime('foo.txt')"
# 2024-11-26 12:17:23 +0100 -> unchanged
```

If the intention was to update timestamps, `FileUtils.touch('foo.txt')`
should be used instead.

@safety
Autocorrection is unsafe for this cop because unlike `File.open`,
`FileUtils.touch` updates an existing file's timestamps.

```ruby
# bad
File.open(filename, 'a') {}
File.open(filename, 'a+') {}

# good
FileUtils.touch(filename)
```

## Options

This rule has no options.

## Blind spots

None recorded.

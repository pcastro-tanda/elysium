# Style/FileEmpty

Prefer to use `File.empty?('path/to/file')` when checking if a file is empty.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Prefer to use `File.empty?('path/to/file')` when checking if a file is empty.

@safety
This cop is unsafe, because `File.size`, `File.read`, and `File.binread`
raise `ENOENT` exception when there is no file corresponding to the path,
while `File.empty?` does not raise an exception.

```ruby
# bad
File.zero?('path/to/file')
File.size('path/to/file') == 0
File.size('path/to/file') >= 0
File.size('path/to/file').zero?
File.read('path/to/file').empty?
File.binread('path/to/file') == ''
FileTest.zero?('path/to/file')

# good
File.empty?('path/to/file')
FileTest.empty?('path/to/file')
```

## Options

This rule has no options.

## Blind spots

None recorded.

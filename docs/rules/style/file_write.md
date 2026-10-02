# Style/FileWrite

Favor `File.(bin)write` convenience methods.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Favor `File.(bin)write` convenience methods.

NOTE: There are different method signatures between `File.write` (class method)
and `File#write` (instance method). The following case will be allowed because
static analysis does not know the contents of the splat argument:

```ruby
File.open(filename, 'w') do |f|
  f.write(*objects)
end
```

```ruby
# bad - text mode
File.open(filename, 'w').write(content)
File.open(filename, 'w') do |f|
  f.write(content)
end

# good
File.write(filename, content)

# bad - binary mode
File.open(filename, 'wb').write(content)
File.open(filename, 'wb') do |f|
  f.write(content)
end

# good
File.binwrite(filename, content)
```

## Options

This rule has no options.

## Blind spots

None recorded.

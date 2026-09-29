# Lint/UnusedBlockArgument

Checks for unused block arguments.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for unused block arguments.

```ruby
# bad
do_something do |used, unused|
  puts used
end

do_something do |bar|
  puts :foo
end

define_method(:foo) do |bar|
  puts :baz
end

# good
do_something do |used, _unused|
  puts used
end

do_something do
  puts :foo
end

define_method(:foo) do |_bar|
  puts :baz
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| IgnoreEmptyBlocks | true |  | Whether to ignore block arguments when the block body is empty. |
| AllowUnusedKeywordArguments | false |  | Whether to allow unused keyword arguments. |

## Blind spots

None recorded.

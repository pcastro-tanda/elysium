# Style/MultilineMemoization

Wrap multiline memoizations in a `begin` and `end` block.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks expressions wrapping styles for multiline memoization.

# EnforcedStyle: keyword (default)

```ruby
# bad
foo ||= (
bar
baz
)

# good
foo ||= begin
bar
baz
end
```

# EnforcedStyle: braces

```ruby
# bad
foo ||= begin
bar
baz
end

# good
foo ||= (
bar
baz
)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `keyword` | `keyword`, `braces` | Whether to wrap multiline memoization blocks in `begin`/`end` keywords or in `(`/`)` parentheses. |

## Blind spots

`Layout/IndentationWidth`'s `Width` is read as a peer option, matching upstream's own `Alignment#configured_indentation_width` cross-cop read.

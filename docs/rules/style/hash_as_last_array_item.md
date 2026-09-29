# Style/HashAsLastArrayItem

Checks for presence or absence of braces around hash literal as a last array item depending on configuration.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for presence or absence of braces around hash literal as a last
array item depending on configuration.

NOTE: This cop will ignore arrays where multiple items are all hashes,
regardless of `EnforcedStyle`.

```ruby
[{ one: 1 }, { two: 2 }]
```

```ruby
# EnforcedStyle: braces (default)

# bad
[1, 2, one: 1, two: 2]

# good
[1, 2, { one: 1, two: 2 }]

# bad
[one: 1, two: 2]

# good
[{ one: 1, two: 2 }]
```

```ruby
# EnforcedStyle: no_braces

# bad
[1, 2, { one: 1, two: 2 }]

# good
[1, 2, one: 1, two: 2]

# bad
[{ one: 1, two: 2 }]

# good
[one: 1, two: 2]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `braces` | `braces`, `no_braces` | Whether a hash literal as the last array item should be wrapped in braces. |

## Blind spots

None recorded.

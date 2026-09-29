# Style/MethodCallWithoutArgsParentheses

Do not use parentheses for method calls with no arguments.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

This cop's allowed methods can be customized with `AllowedMethods`. By default,
there are no allowed methods.

NOTE: This cop allows the use of `it()` without arguments in blocks, as in
`0.times { it() }`, following `Lint/ItWithoutArgumentsInBlock`.

```ruby
# bad
object.some_method()

# good
object.some_method
```

With `AllowedMethods: []` (default):

```ruby
# bad
object.foo()
```

With `AllowedMethods: [foo]`:

```ruby
# good
object.foo()
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `[]` |  | Method names always allowed to keep empty parentheses. |
| AllowedPatterns | `[]` |  | Method name regex patterns always allowed to keep empty parentheses. |

## Blind spots

`AllowedPatterns` entries that fail to compile as a Rust regex are dropped (never match) rather than raising a configuration error.

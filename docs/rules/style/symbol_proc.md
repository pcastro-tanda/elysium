# Style/SymbolProc

Use symbols as procs instead of blocks when possible.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | nursery |

If you prefer a style that allows a block for a method with arguments,
set `true` for `AllowMethodsWithArguments`. `define_method` is allowed by
default; customize with `AllowedMethods`/`AllowedPatterns`.

```ruby
# bad
something.map { |s| s.upcase }
something.map { _1.upcase }
something.map { it.upcase }

# good
something.map(&:upcase)
```

With `AllowMethodsWithArguments: false` (default):

```ruby
# bad
something.do_something(foo) { |o| o.bar }

# good
something.do_something(foo, &:bar)
```

With `AllowComments: true`, a block/lambda with a comment anywhere in its
body is left alone even though it would otherwise be flagged.

With `AllCops: ActiveSupportExtensionsEnabled: true`, `->(x) { x.foo }`,
`proc { |x| x.foo }`, and `Proc.new { |x| x.foo }` are all left alone (their
behavior differs from a symbol-to-proc once ActiveSupport is loaded).

@safety
This cop is unsafe: a `Proc` from `Symbol#to_proc` behaves like a lambda
(strict arity, `ArgumentError` on a wrong argument count) where a `Proc`
from a block does not, and `Symbol#to_proc` cannot call a `protected`
method that would otherwise be accessible.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowMethodsWithArguments | false |  | Allow the block form for a method call that itself has arguments. |
| AllowedMethods | `define_method` |  | Method names always allowed to take a block instead of a symbol proc. |
| AllowedPatterns | `[]` |  | Method name regex patterns always allowed to take a block. |
| AllowComments | false |  | Allow a block/lambda whose body contains a comment. |

## Blind spots

`AllowedPatterns` entries that fail to compile as a regex are dropped
(never match) rather than raising a configuration error.
The fix's leading-whitespace trim (RuboCop's
`range_with_surrounding_space(side: :left)`) recognizes a run of spaces/tabs
then a run of `\n`, matching Unix line endings; it does not special-case a
preceding `\r` (CRLF sources) or a `\`-newline continuation.
Cross-cop autocorrection ordering (RuboCop's
`autocorrect_incompatible_with: [Layout::SpaceBeforeBlockBraces]`) is not
replicated; it only matters when both cops run in the same fix pass.

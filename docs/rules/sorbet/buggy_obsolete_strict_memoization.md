# Sorbet/BuggyObsoleteStrictMemoization

Checks for the a mistaken variant of the "obsolete memoization pattern" that used to be required for older Sorbet versions in `#typed: strict` files.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for the a mistaken variant of the "obsolete memoization pattern" that used to be required
for older Sorbet versions in `#typed: strict` files. The mistaken variant would overwrite the ivar with `nil`
on every call, causing the memoized value to be discarded and recomputed on every call.

This cop will correct it to read from the ivar instead of `nil`, which will memoize it correctly.

The result of this correction will be the "obsolete memoization pattern", which can further be corrected by
the `Sorbet/ObsoleteStrictMemoization` cop.

See `Sorbet/ObsoleteStrictMemoization` for more details.

If the computation being memoized had side effects, calling it only once (instead of once on every call
to the affected method) can be observed, and might be a breaking change.

```ruby
# bad
sig { returns(Foo) }
def foo
  @foo = T.let(nil, T.nilable(Foo))
  @foo ||= some_computation
end

# good
sig { returns(Foo) }
def foo
  @foo = T.let(@foo, T.nilable(Foo))
  @foo ||= some_computation
end
```

## Options

This rule has no options.

## Blind spots

None recorded.

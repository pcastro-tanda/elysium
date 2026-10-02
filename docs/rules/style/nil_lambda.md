# Style/NilLambda

Prefer `-> {}` to `-> { nil }`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for lambdas and procs that always return nil, which can be replaced with an empty lambda or proc instead.

NOTE: A `proc` that returns nil via an explicit `return` is allowed, because in a `proc` `return` exits the enclosing method, so removing it would change behavior. A lambda is still reported, since there `return` only exits the lambda itself.

## Options

This rule has no options.

## Blind spots

None recorded.

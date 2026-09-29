# Style/DoubleNegation

Checks for uses of double negation (!!).

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for uses of double negation (`!!`) to convert something to a boolean value.

When using `EnforcedStyle: allowed_in_returns`, allow double negation in contexts
that use boolean as a return value. When using `EnforcedStyle: forbidden`, double
negation should be forbidden always.

NOTE: when `something` is a boolean value `!!something` and `!something.nil?` are
not the same thing. As you're unlikely to write code that can accept values of any
type this is rarely a problem in practice.

@safety
  Autocorrection is unsafe when the value is `false`, because the result of the
  expression will change (`!!false #=> false`, `!false.nil? #=> true`).

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `allowed_in_returns` | `allowed_in_returns`, `forbidden` | Whether `!!` at an implicit method return is allowed. |

## Blind spots

`find_last_child`'s single-statement digestion (whitequark elides a `begin`
wrapper for exactly one statement, so `child_nodes.last` actually descends one
level into that statement's own last child) is only replicated for the
shapes the corpus exercises -- `ArrayNode`/`HashNode` and `IfNode`/
`UnlessNode` bodies; any other single non-collection, non-conditional
statement is treated as its own last child instead of digging further into
its structure, which only risks the false-negative (exempt) direction and
happens to match every fixture, since such a statement always textually
contains the flagged node itself.

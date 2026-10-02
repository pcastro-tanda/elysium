# Lint/UnexpectedBlockArity

Looks for blocks that have fewer arguments that the calling method expects.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for a block that is known to need more positional block arguments than are given (by default this is configured for `Enumerable` methods needing 2 arguments). Optional arguments are allowed, although they don't generally make sense as the default value will be used. Blocks that have no receiver, or take splatted arguments (ie. `*args`) are always accepted.

Keyword arguments (including `**kwargs`) do not get counted towards this, as they are not used by the methods in question.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Methods | `nil` |  | Method names and their expected minimum positional block arity. |

## Blind spots

None recorded.

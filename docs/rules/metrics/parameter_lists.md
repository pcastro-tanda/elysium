# Metrics/ParameterLists

Avoid parameter lists longer than three or four parameters.

| | |
| --- | --- |
| Department | Metrics |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for methods with too many parameters.

The maximum number of parameters is configurable. Keyword arguments can optionally be excluded from the total count, as they add less complexity than positional or optional parameters.

Any number of arguments for `initialize` inside a block of `Struct.new` or `Data.define` is always allowed, since checking the number of arguments of that `initialize` method does not make sense.

NOTE: An explicit block argument (`&block`) is never counted, to prevent an erroneous change that is avoided by making the block argument implicit.

This cop also checks for the maximum number of optional parameters, configurable via `MaxOptionalParameters`.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Max | 5 |  | Maximum number of parameters allowed. |
| CountKeywordArgs | true |  | Count keyword args towards the maximum. |
| MaxOptionalParameters | 3 |  | Maximum number of optional parameters allowed. |

## Blind spots

None recorded.

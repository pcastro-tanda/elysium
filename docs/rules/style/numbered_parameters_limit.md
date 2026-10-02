# Style/NumberedParametersLimit

Avoid excessive numbered params in a single block.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Detects use of an excessive amount of numbered parameters in a single block. Having too many numbered parameters can make code too cryptic and hard to read.

The cop defaults to registering an offense if there is more than 1 numbered parameter but this maximum can be configured by setting `Max`.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Max | 1 |  | Maximum number of distinct numbered parameters (`_1`..`_9`) a single block may use. |

## Blind spots

None recorded.

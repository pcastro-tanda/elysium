# Metrics/AbcSize

Checks that the ABC size of methods is not higher than the configured maximum.

| | |
| --- | --- |
| Department | Metrics |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

The ABC size is based on assignments, branches (method calls), and conditions. See https://wiki.c2.com/?AbcMetric and https://en.wikipedia.org/wiki/ABC_Software_Metric.

Interpreting ABC size:

* `<= 17` satisfactory
* `18..30` unsatisfactory
* `> 30` dangerous

You can have repeated "attributes" calls count as a single "branch". For this purpose, attributes are any method with no argument; no attempt is meant to distinguish actual `attr_reader` from other methods.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Max | 17 |  | Maximum ABC size allowed. |
| CountRepeatedAttributes | true |  | Count each repeated attribute call as its own branch. |
| AllowedMethods | `[]` |  | Method names to exempt. |
| AllowedPatterns | `[]` |  | Patterns matching method names to exempt. |

## Blind spots

None recorded.

# Naming/RescuedExceptionsVariableName

Use consistent rescued exceptions variables naming.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

The `PreferredName` config option takes a `String`. It represents the required name of the variable. Its default is `e`.

This cop does not consider nested rescues because it cannot guarantee that the variable from the outer rescue is not used within the inner rescue (in which case, changing the inner variable would shadow the outer variable).

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| PreferredName | `e` |  | The required name of the variable. |

## Blind spots

None recorded.

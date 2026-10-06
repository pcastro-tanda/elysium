# ThreadSafety/ClassAndModuleAttributes

Avoid mutating class and module attributes.

| | |
| --- | --- |
| Department | ThreadSafety |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Avoid mutating class and module attributes. They are implemented by class variables, which are not thread-safe.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| ActiveSupportClassAttributeAllowed | false |  | Allow `class_attribute` (ActiveSupport). |

## Blind spots

None recorded.

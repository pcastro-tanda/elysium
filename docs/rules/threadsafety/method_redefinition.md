# ThreadSafety/MethodRedefinition

Do not use `remove_method` followed by method definition.

| | |
| --- | --- |
| Department | ThreadSafety |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Avoid the thread-unsafe combination of remove_method followed by defining a method with the same name. This can lead to a race condition, as these two actions are not atomic. As a safer alternative, consider aliasing the method to itself instead.

## Options

This rule has no options.

## Blind spots

None recorded.

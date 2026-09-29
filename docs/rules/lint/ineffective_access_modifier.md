# Lint/IneffectiveAccessModifier

Checks for attempts to use `private` or `protected` to set the visibility of a class method, which does not work.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

`private` or `protected` access modifiers which are applied to a singleton
method do not make singleton methods private/protected. `private_class_method`
can be used for that.

## Options

This rule has no options.

## Blind spots

None recorded.

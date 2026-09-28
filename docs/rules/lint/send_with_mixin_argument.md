# Lint/SendWithMixinArgument

Checks for `send` method when using mixin.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

`include` and `prepend` methods were private methods until Ruby 2.0, they were mixed-in via `send` method. This cop uses Ruby 2.1 or higher style that can be called by public methods. And `extend` method that was originally a public method is also targeted for style unification.

## Options

This rule has no options.

## Blind spots

None recorded.

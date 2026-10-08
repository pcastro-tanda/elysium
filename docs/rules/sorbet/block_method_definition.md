# Sorbet/BlockMethodDefinition

Disallows defining methods inside blocks without using `define_method`, unless the block is a named class definition. This is to avoid running into https://github.com/sorbet/sorbet/issues/3609.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |



## Options

This rule has no options.

## Blind spots

None recorded.

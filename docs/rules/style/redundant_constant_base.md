# Style/RedundantConstantBase

Avoid redundant `::` prefix on constant.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Avoid redundant `::` prefix on a constant.

How Ruby searches constants is a bit complicated, and it can often be difficult to understand from the code whether the `::` is intended or not. Where `Module.nesting` is empty, there is no need to prepend `::`, so it would be nice to consistently avoid such meaningless `::` prefix to avoid confusion.

NOTE: This cop is disabled if `Lint/ConstantResolution` cop is enabled, to prevent conflicting rules. This is because it respects user configurations that want to enable `Lint/ConstantResolution` cop which is disabled by default.

## Options

This rule has no options.

## Blind spots

Does not implement `AllCops/UseProjectIndex`'s `provably_unshadowed?` (rubydex-backed resolution), nor a leading `::` on one target of a multiple assignment (`ConstantPathTargetNode`); no fixture exercises either.

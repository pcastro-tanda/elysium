# Performance/AncestorsInclude

Use `A <= B` instead of `A.ancestors.include?(B)`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies usages of `ancestors.include?` and change them to use `<=` instead.

This cop is unsafe because it can't tell whether the receiver is a class or an object.

## Options

This rule has no options.

## Blind spots

None recorded.

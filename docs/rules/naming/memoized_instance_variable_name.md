# Naming/MemoizedInstanceVariableName

Memoized method name should match memo instance variable name.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for memoized methods whose instance variable name does not match the method name. Applies to both regular methods (defined with `def`) and dynamic methods (defined with `define_method` or `define_singleton_method`).

This cop can be configured with the `EnforcedStyleForLeadingUnderscores` directive. It can be configured to allow for memoized instance variables prefixed with an underscore. Prefixing ivars with an underscore is a convention that is used to implicitly indicate that an ivar should not be set or referenced outside of the memoization method.

This cop relies on the pattern `@instance_var ||= ...`, but this is sometimes used for other purposes than memoization so this cop is considered unsafe. Also, its autocorrection is unsafe because it may conflict with instance variable names already in use.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyleForLeadingUnderscores | `disallowed` | `disallowed`, `required`, `optional` | Whether memoized instance variables are required, allowed, or forbidden to have a leading underscore. |

## Blind spots

None recorded.

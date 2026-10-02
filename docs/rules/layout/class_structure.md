# Layout/ClassStructure

Checks if the code style follows the `ExpectedOrder` configuration.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

`Categories` allows mapping macro names into a category. Consider an example of code style that covers the following order: module inclusion (`include`, `prepend`, `extend`), constants, associations, public attribute macros, other macros, public class methods, the initializer, public instance methods, protected attribute macros and methods, then private attribute macros and methods. Simply enabling the cop with `Enabled: true` does not use that example order --`ExpectedOrder` and `Categories` must both be configured for macro ordering (e.g. `attr_reader`) to be enforced.

Autocorrection is unsafe because class methods and module inclusion can behave differently based on which methods or constants have already been defined; constants are only moved when assigned a literal.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| ExpectedOrder | `module_inclusion`, `constants`, `public_class_methods`, `initializer`, `public_methods`, `protected_methods`, `private_class_methods`, `private_methods` |  | The order classes and modules should be structured in. |
| Categories | `nil` |  | A hash mapping a category name to the list of method names grouped under it, for `ExpectedOrder` purposes. Default: `{"module_inclusion": ["include", "prepend", "extend"]}`. |

## Blind spots

Sibling adjacency for the autocorrect anchor/barrier/movable-group search is derived from this cop's own flattened direct-element list rather than real Prism parent/child relationships, so a misordered element nested inside an explicit `begin...end` grouping block can never be reordered across that block's boundary (no fixture exercises this either way).

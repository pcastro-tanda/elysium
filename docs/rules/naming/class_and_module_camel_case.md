# Naming/ClassAndModuleCamelCase

Use CamelCase for classes and modules.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for class and module names with an underscore in them.

`AllowedNames` config takes an array of permitted names. Its default value is `['module_parent']`. These names can be full class/module names or part of the name. eg. Adding `my_class` to the `AllowedNames` config will allow names like `my_class`, `my_class::User`, `App::my_class`, `App::my_class::User`, etc.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedNames | `module_parent` |  | Permitted class/module names (full or partial). |

## Blind spots

None recorded.

# Layout/EmptyLineBetweenDefs

Use empty lines between class/module/method defs.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks whether class/module/method definitions are separated by one or more empty lines.

`NumberOfEmptyLines` can be an integer (default is 1) or an array (e.g. `[1, 2]`) to specify a minimum and maximum number of empty lines permitted.

`AllowAdjacentOneLineDefs` configures whether adjacent one-line definitions are considered an offense.

```ruby
# bad
def a
end
def b
end

# good
def a
end

def b
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EmptyLineBetweenMethodDefs | true |  | Checks for empty lines between method definitions. |
| EmptyLineBetweenClassDefs | true |  | Checks for empty lines between class definitions. |
| EmptyLineBetweenModuleDefs | true |  | Checks for empty lines between module definitions. |
| DefLikeMacros | `[]` |  | The name of any macro that you want to treat like a def. |
| AllowAdjacentOneLineDefs | true |  | Whether single line method definitions need an empty line between them. |
| NumberOfEmptyLines | 1 |  | Can be an array to specify a minimum and maximum number of empty lines, e.g. `[1, 2]`. |

## Blind spots

`DefLikeMacros` candidacy approximates RuboCop-AST's `in_macro_scope?` (a receiver-less call sits in a class/module/singleton-class body, or at the program's top level, tunneling through `if`/`unless`/`begin`/block wrappers) rather than reproducing its exact recursive node pattern; a `Class.new`/`Struct.new` block body (RuboCop's `class_constructor?`) is not specially recognized as class-like, only as a transparent block wrapper (matches in practice, since that branch of RuboCop's own pattern also just tunnels through `any_block`).

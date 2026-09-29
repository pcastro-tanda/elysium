# Style/GlobalVars

Do not introduce global variables.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Looks for uses of global variables. Global variables introduce
shared mutable state that makes code harder to test, debug,
and reason about, since any part of the program can read or modify them.

It does not report offenses for built-in global variables.
Built-in global variables are allowed by default. Additionally
users can allow additional variables via the AllowedVariables option.

Note that backreferences like $1, $2, etc are not global variables.

```ruby
# bad
$foo = 2
bar = $foo + 5

# good
FOO = 2
foo = 2
$stdin.read
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedVariables | `[]` |  | Allowed global variables. |

## Blind spots

None recorded.

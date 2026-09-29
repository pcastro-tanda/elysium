# Style/StabbyLambdaParentheses

Checks for the usage of parentheses around stabby lambda arguments.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for parentheses around stabby lambda arguments. There are two different styles. Defaults to `require_parentheses`.

# bad
->a,b,c { a + b + c }

# good
->(a,b,c) { a + b + c}

# EnforcedStyle: require_no_parentheses

# bad
->(a,b,c) { a + b + c }

# good
->a,b,c { a + b + c}

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `require_parentheses` | `require_parentheses`, `require_no_parentheses` | Whether to require parentheses around stabby lambda arguments. |

## Blind spots

None recorded.

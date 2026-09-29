# Style/MethodDefParentheses

Checks if the method definitions have or don't have parentheses.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for parentheses around the arguments in method definitions. Both instance and class/singleton methods are checked.

Regardless of style, parentheses are necessary for:

1. Endless methods
2. Argument lists containing a `forward-arg` (`...`)
3. Argument lists containing an anonymous rest arguments forwarding (`*`)
4. Argument lists containing an anonymous keyword rest arguments forwarding (`**`)
5. Argument lists containing an anonymous block forwarding (`&`)

Removing the parens would be a syntax error here.

```ruby
# EnforcedStyle: require_parentheses (default)
# The `require_parentheses` style requires method definitions
# to always use parentheses

# bad
def bar num1, num2
  num1 + num2
end

# good
def bar(num1, num2)
  num1 + num2
end

# EnforcedStyle: require_no_parentheses
# The `require_no_parentheses` style requires method definitions
# to never use parentheses

# bad
def bar(num1, num2)
  num1 + num2
end

# good
def bar num1, num2
  num1 + num2
end

# EnforcedStyle: require_no_parentheses_except_multiline
# The `require_no_parentheses_except_multiline` style prefers no
# parentheses when method definition arguments fit on single line,
# but prefers parentheses when arguments span multiple lines.

# bad
def bar(num1, num2)
  num1 + num2
end

# good
def bar num1, num2
  num1 + num2
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `require_parentheses` | `require_parentheses`, `require_no_parentheses`, `require_no_parentheses_except_multiline` | Whether method definitions require parentheses around their arguments. |

## Blind spots

None recorded.

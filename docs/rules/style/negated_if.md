# Style/NegatedIf

Favor unless over if for negative conditions (or control flow or).

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of if with a negated condition. Only ifs
without else are considered. There are three different styles:

* both
* prefix
* postfix

```ruby
# EnforcedStyle: both (default)
# enforces `unless` for `prefix` and `postfix` conditionals

# bad

if !foo
  bar
end

# good

unless foo
  bar
end

# bad

bar if !foo

# good

bar unless foo

# EnforcedStyle: prefix
# enforces `unless` for just `prefix` conditionals

# bad

if !foo
  bar
end

# good

unless foo
  bar
end

# good

bar if !foo

# EnforcedStyle: postfix
# enforces `unless` for just `postfix` conditionals

# bad

bar if !foo

# good

bar unless foo

# good

if !foo
  bar
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `both` | `both`, `prefix`, `postfix` | `both`: prefix and postfix negated `if` should both use `unless`. `prefix`: only use `unless` for negated `if` statements positioned before the body of the statement. `postfix`: only use `unless` for negated `if` statements positioned after the body of the statement. |

## Blind spots

None recorded.

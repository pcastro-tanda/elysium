# Style/NegatedUnless

Favor if over unless for negative conditions.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of `unless` with a negated condition. Only `unless` without `else` are considered. There are three different styles:

* `both`
* `prefix`
* `postfix`

```ruby
# EnforcedStyle: both (default)
# enforces `if` for `prefix` and `postfix` conditionals

# bad
unless !foo
  bar
end

# good
if foo
  bar
end

# bad
bar unless !foo

# good
bar if foo

# EnforcedStyle: prefix
# enforces `if` for just `prefix` conditionals

# bad
unless !foo
  bar
end

# good
if foo
  bar
end

# good
bar unless !foo

# EnforcedStyle: postfix
# enforces `if` for just `postfix` conditionals

# bad
bar unless !foo

# good
bar if foo

# good
unless !foo
  bar
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `both` | `both`, `prefix`, `postfix` | Whether to flag negated conditions in both prefix (`unless ... end`) and postfix (`... unless ...`) `unless`, or just one of the two forms. |

## Blind spots

None recorded.

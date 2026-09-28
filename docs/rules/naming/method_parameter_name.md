# Naming/MethodParameterName

Checks method parameter names for how descriptive they are.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

It is highly configurable.

The `MinNameLength` config option takes an integer. It represents the
minimum amount of characters the name must be. Its default is 3. The
`AllowNamesEndingInNumbers` config option takes a boolean. When set to
false, this cop will register offenses for names ending with numbers. Its
default is true. The `AllowedNames` config option takes an array of
permitted names that will never register an offense. The `ForbiddenNames`
config option takes an array of restricted names that will always
register an offense.

```ruby
# bad
def bar(varOne, varTwo)
  varOne + varTwo
end

# With `AllowNamesEndingInNumbers` set to false
def foo(num1, num2)
  num1 * num2
end

# With `MinNameLength` set to number greater than 1
def baz(a, b, c)
  do_stuff(a, b, c)
end

# good
def bar(thud, fred)
  thud + fred
end

def foo(speed, distance)
  speed * distance
end

def baz(age_a, height_b, gender_c)
  do_stuff(age_a, height_b, gender_c)
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| MinNameLength | 3 |  | Minimum number of characters a method parameter name must have. |
| AllowNamesEndingInNumbers | true |  | Whether a method parameter name may end with a digit. |
| AllowedNames | `as`, `at`, `by`, `cc`, `db`, `id`, `if`, `in`, `io`, `ip`, `of`, `on`, `os`, `pp`, `to` |  | Method parameter names that are never flagged. |
| ForbiddenNames | `[]` |  | Method parameter names that are always flagged. |

## Blind spots

None recorded.

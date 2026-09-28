# Style/OptionalArguments

Checks for optional arguments to methods that do not come at the end of the argument list.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for optional arguments to methods
that do not come at the end of the argument list.

```ruby
# bad
def foo(a = 1, b, c)
end

# good
def baz(a, b, c = 1)
end

def foobar(a = 1, b = 2, c = 3)
end
```

## Options

This rule has no options.

## Blind spots

This cop is unsafe: changing a method signature implicitly changes call-site behaviour. RuboCop
reports it regardless (there is no `safe_autocorrect` opt-out and no autocorrection is offered); this
port matches that -- it never suppresses the offense on safety grounds and, like upstream, does not
attempt to fix it.

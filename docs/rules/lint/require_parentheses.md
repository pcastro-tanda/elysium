# Lint/RequireParentheses

Checks for expressions where there is a call to a predicate method with at least one argument, where no parentheses are used around the parameter list, and a boolean operator, && or ||, is used in the last argument.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for expressions where there is a call to a predicate
method with at least one argument, where no parentheses are used around
the parameter list, and a boolean operator, && or ||, is used in the
last argument.

The idea behind warning for these constructs is that the user might
be under the impression that the return value from the method call is
an operand of &&/||.

```ruby
# bad
if day.is? :tuesday && month == :jan
  # ...
end

# good
if day.is?(:tuesday) && month == :jan
  # ...
end
```

## Options

This rule has no options.

## Blind spots

Structural equality checks fall back on this codebase's usual exact source-text comparison, but
this cop performs none itself; nothing else is known to diverge from upstream.

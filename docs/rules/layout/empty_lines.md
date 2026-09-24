# Layout/EmptyLines

Checks for two or more consecutive blank lines.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad - it has two empty lines.
some_method
# one empty line
# two empty lines
some_method

# good
some_method
# one empty line
some_method
```

## Options

This rule has no options.

## Blind spots

Blank lines are never flagged inside a multi-line string, x-string or
regexp literal (matching RuboCop's per-line lexer tokens for those). The
same per-line-token behaviour also exempts `%w`/`%i` word/symbol arrays and
plain arrays/method calls from ever gaining a synthetic token for a blank
line, which this port already treats correctly by leaving them untouched -
their blank lines are ordinary candidates, exactly as in real RuboCop.
Percent-literal arrays (`%w`, `%i`, `%W`, `%I`) and other multi-line
constructs are not modelled as literal spans because RuboCop's own lexer
does not special-case them either.

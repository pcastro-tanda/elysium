# Lint/LambdaWithoutLiteralBlock

Checks uses of lambda without a literal block.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

```ruby
# bad
lambda(&proc { do_something })
lambda(&Proc.new { do_something })

# good
proc { do_something }
Proc.new { do_something }
lambda { do_something } # If you use lambda.
```

## Options

This rule has no options.

## Blind spots

None recorded.

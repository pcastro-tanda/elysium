# Layout/SpaceAroundMethodCallOperator

Checks method call operators to not have spaces around them.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
foo. bar
foo .bar
foo . bar
foo. bar .buzz
foo
  . bar
  . buzz
foo&. bar
foo &.bar
foo &. bar
foo &. bar&. buzz
RuboCop:: Cop
RuboCop:: Cop:: Base
:: RuboCop::Cop

# good
foo.bar
foo.bar.buzz
foo
  .bar
  .buzz
foo&.bar
foo&.bar&.buzz
RuboCop::Cop
RuboCop::Cop::Base
::RuboCop::Cop
```

## Options

This rule has no options.

## Blind spots

None recorded.

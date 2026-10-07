# Minitest/RefuteRespondTo

This cop enforces the test to use `refute_respond_to(object, :do_something)` over `refute(object.respond_to?(:do_something))`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the test to use `refute_respond_to(object, :do_something)` over `refute(object.respond_to?(:do_something))`.

## Options

This rule has no options.

## Blind spots

None recorded.

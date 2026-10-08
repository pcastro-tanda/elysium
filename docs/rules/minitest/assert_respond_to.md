# Minitest/AssertRespondTo

This cop enforces the test to use `assert_respond_to(object, :do_something)` over `assert(object.respond_to?(:do_something))`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of `assert_respond_to(object, :do_something)` over `assert(object.respond_to?(:do_something))`.

## Options

This rule has no options.

## Blind spots

None recorded.

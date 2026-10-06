# Minitest/TestFileName

Checks if test file names start with `test_` or end with `_test.rb`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks if test file names start with `test_` or end with `_test.rb`. Files which define classes having names ending with `Test` are checked. Not following this convention may result in tests not being run.

```ruby
# bad
my_class.rb

# good
my_class_test.rb
test_my_class.rb
```

## Options

This rule has no options.

## Blind spots

None recorded.

# Minitest/GlobalExpectations

This cop checks for deprecated global expectations.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for deprecated global expectations and autocorrects them to use expect format.

```ruby
# EnforcedStyle: any (default)
# bad
musts.must_equal expected_musts
wonts.wont_match expected_wonts
musts.must_raise TypeError

# good
_(musts).must_equal expected_musts
_(wonts).wont_match expected_wonts
_ { musts }.must_raise TypeError

expect(musts).must_equal expected_musts
expect(wonts).wont_match expected_wonts
expect { musts }.must_raise TypeError

value(musts).must_equal expected_musts
value(wonts).wont_match expected_wonts
value { musts }.must_raise TypeError
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `any` | `_`, `any`, `expect`, `value` | Which spelling of the expectation DSL receiver to enforce. |

## Blind spots

None recorded.

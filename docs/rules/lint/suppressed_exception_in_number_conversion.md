# Lint/SuppressedExceptionInNumberConversion

Checks for cases where exceptions unrelated to the numeric constructors may be unintentionally swallowed.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for cases where exceptions unrelated to the numeric constructors `Integer()`, `Float()`, `BigDecimal()`, `Complex()`, and `Rational()` may be unintentionally swallowed.

```ruby
# bad
Integer(arg) rescue nil

# bad
begin
  Integer(arg)
rescue
  nil
end

# good
Integer(arg, exception: false)
```

This cop's autocorrection is unsafe because unexpected errors occurring in the argument passed to the numeric constructor (e.g., `Integer()`) can lead to incompatible behavior. For example, changing `Integer(arg) rescue nil` to `Integer(arg, exception: false)` ensures that exceptions raised by `arg` itself are not ignored.

## Options

This rule has no options.

## Blind spots

None recorded.

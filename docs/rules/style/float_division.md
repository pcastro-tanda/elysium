# Style/FloatDivision

For performing float division, coerce one side only.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for division with integers coerced to floats.
It is recommended to either always use `fdiv` or coerce one side only.
This cop also provides other options for code consistency.

For `Regexp.last_match` and nth reference (e.g., `$1`), it assumes that the value
is a string matched by a regular expression, and allows conversion with `#to_f`.

@safety
  This cop is unsafe, because if the operand variable is a string object
  then `#to_f` will be removed and an error will occur.

  ```ruby
  a = '1.2'
  b = '3.4'
  a.to_f / b.to_f # Both `to_f` calls are required here
  ```

With `EnforcedStyle: single_coerce` (default):

```ruby
# bad
a.to_f / b.to_f

# good
a.to_f / b
a / b.to_f
```

With `EnforcedStyle: left_coerce`:

```ruby
# bad
a / b.to_f
a.to_f / b.to_f

# good
a.to_f / b
```

With `EnforcedStyle: right_coerce`:

```ruby
# bad
a.to_f / b
a.to_f / b.to_f

# good
a / b.to_f
```

With `EnforcedStyle: fdiv`:

```ruby
# bad
a / b.to_f
a.to_f / b
a.to_f / b.to_f

# good
a.fdiv(b)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `single_coerce` | `left_coerce`, `right_coerce`, `single_coerce`, `fdiv` | Which side(s) of a float division to coerce. |

## Blind spots

Upstream's `offense_condition?` reaches `node.receiver.receiver`/
`node.first_argument.receiver` through rubocop-ast's generic `Node#receiver`
node-matcher, which also matches a block-wrapping-a-call receiver
(`any_block (call $_ ...)`); this port's `generic_receiver` only implements
the plain-call half, so a division operand that is itself a block-carrying
call (e.g. `foo.bar { }.to_f / baz`) is not walked through to its own
receiver for the `Regexp.last_match`/nth-ref exemption. No fixture or spec
example exercises that shape.

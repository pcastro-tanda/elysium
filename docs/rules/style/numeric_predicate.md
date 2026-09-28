# Style/NumericPredicate

Checks for the use of predicate- or comparison methods for numeric comparisons.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for usage of comparison operators (`==`, `>`, `<`) to test numbers as
zero, positive, or negative. These can be replaced by their respective
predicate methods. This cop can also be configured to do the reverse.

This cop's allowed methods can be customized with `AllowedMethods`. By
default, there are no allowed methods.

This cop disregards `#nonzero?` as its value is truthy or falsey, but not
`true` and `false`, and thus not always interchangeable with `!= 0`.

This cop allows comparisons to global variables, since they are often
populated with objects which can be compared with integers, but are not
themselves `Integer` polymorphic.

@safety
  This cop is unsafe because it cannot be guaranteed that the receiver
  defines the predicates or can be compared to a number, which may lead to a
  false positive for non-standard classes.

With `EnforcedStyle: predicate` (default):

```ruby
# bad
foo == 0
0 > foo
bar.baz > 0

# good
foo.zero?
foo.negative?
bar.baz.positive?
```

With `EnforcedStyle: comparison`:

```ruby
# bad
foo.zero?
foo.negative?
bar.baz.positive?

# good
foo == 0
0 > foo
bar.baz > 0
```

With `AllowedMethods: [==]` and `EnforcedStyle: predicate`:

```ruby
# good
foo == 0

# bad
0 > foo
bar.baz > 0
```

With `AllowedPatterns: ['zero']` and `EnforcedStyle: predicate`:

```ruby
# bad
foo.zero?

# bad
foo.negative?
bar.baz.positive?
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `predicate` | `predicate`, `comparison` | Whether to prefer predicate methods (`foo.zero?`) or comparisons (`foo == 0`). |
| AllowedMethods | `[]` |  | Method names always allowed to take the disfavored form, checked against the matched call's own name and every enclosing call's name. |
| AllowedPatterns | `[]` |  | Method name regex patterns always allowed to take the disfavored form, checked the same way as `AllowedMethods`. |

## Blind spots

`AllowedPatterns` entries that fail to compile as a Rust regex are dropped (never match) rather than raising a configuration error.

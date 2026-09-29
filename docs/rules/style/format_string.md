# Style/FormatString

Enforce the use of Kernel#sprintf, Kernel#format or String#%.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of a single string formatting utility. Valid options
include `Kernel#format`, `Kernel#sprintf`, and `String#%`.

The detection of `String#%` cannot be implemented in a reliable manner for
all cases, so only two scenarios are considered -- if the first argument is
a string literal and if the second argument is an array literal.

Autocorrection will be applied when the argument is a literal or uses a
known built-in conversion method such as `to_d`, `to_f`, `to_h`, `to_i`,
`to_r`, `to_s`, and `to_sym` on variables, provided that their return value
is not an array. For example, when using `to_s`, `'%s' % [1, 2, 3].to_s`
can be autocorrected without any incompatibility:

```ruby
'%s' % [1, 2, 3]        #=> '1'
format('%s', [1, 2, 3]) #=> '[1, 2, 3]'
'%s' % [1, 2, 3].to_s   #=> '[1, 2, 3]'
```

With `EnforcedStyle: format` (default):

```ruby
# bad
puts sprintf('%10s', 'foo')
puts '%10s' % 'foo'

# good
puts format('%10s', 'foo')
```

With `EnforcedStyle: sprintf`:

```ruby
# bad
puts format('%10s', 'foo')
puts '%10s' % 'foo'

# good
puts sprintf('%10s', 'foo')
```

With `EnforcedStyle: percent`:

```ruby
# bad
puts format('%10s', 'foo')
puts sprintf('%10s', 'foo')

# good
puts '%10s' % 'foo'
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `format` | `format`, `sprintf`, `percent` | Which formatting utility to enforce. |

## Blind spots

Only three call shapes are ever considered: a receiverless `format`/
`sprintf` call with at least two arguments, `str_or_dstr % arg` for any
`arg`, and `anything_else % (array_or_hash_literal)`. A `%` call whose
receiver's type cannot be inferred from its own literal syntax (e.g. through
a local variable or method call known by other means to return a `String`)
is not flagged unless its RHS happens to be an array or hash literal,
matching RuboCop's own documented limitation.

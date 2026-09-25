# Lint/NumberConversion

Warns the usage of unsafe number conversions.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | unsafe |
| Stability | nursery |

Warns the usage of unsafe number conversions. Unsafe number conversion can
cause unexpected error if auto type conversion fails. This cop prefers
parsing with a number class instead.

Conversion with `Integer`, `Float`, etc. will raise an `ArgumentError` if
given input that is not numeric (e.g. an empty string), whereas `to_i`, etc.
will try to convert regardless of input (`''.to_i => 0`). As such, this cop
is disabled by default because it's not necessarily always correct to raise
if a value is not numeric.

NOTE: Some values cannot be converted properly using one of the `Kernel`
methods (for instance, `Time` and `DateTime` values are allowed by this cop
by default). Similarly, Rails' duration methods do not work well with
`Integer()` and can be allowed with `AllowedMethods`. By default, there are
no methods allowed.

```ruby
# bad
'10'.to_i
'10.2'.to_f
'10'.to_c
'1/3'.to_r
['1', '2', '3'].map(&:to_i)
foo.try(:to_f)
bar.send(:to_c)

# good
Integer('10', 10)
Float('10.2')
Complex('10')
Rational('1/3')
['1', '2', '3'].map { |i| Integer(i, 10) }
foo.try { |i| Float(i) }
bar.send { |i| Complex(i) }
```

With `AllowedMethods: [minutes]` (default: `[]`):

```ruby
# good
10.minutes.to_i
```

With `AllowedPatterns: ['min*']` (default: `[]`):

```ruby
# good
10.minutes.to_i
```

With `IgnoredClasses: [Time, DateTime]` (the default):

```ruby
# good
Time.now.to_datetime.to_i
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `[]` |  | Method names on a `to_i`/etc. receiver that are always allowed. |
| AllowedPatterns | `[]` |  | Method name regex patterns on a `to_i`/etc. receiver that are always allowed. |
| IgnoredClasses | `Time`, `DateTime` |  | Top-level constant receivers never flagged. |

## Blind spots

Upstream's `to_method_symbol` node pattern binds a `receiver` block parameter
that is actually the *outer* call's own method name (never `nil`, given the
pattern's child order), so its `next if receiver.nil?` guard never fires in
practice; this port therefore only implements the `node.arguments.one?`
guard that does. `AllowedMethods`/`AllowedPatterns` are matched against the
receiver call's own method name, not its full source text.

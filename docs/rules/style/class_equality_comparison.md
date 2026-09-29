# Style/ClassEqualityComparison

Enforces the use of `Object#instance_of?` instead of class comparison for equality.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

`==`, `equal?`, and `eql?` custom method definitions are allowed by default. These are customizable with the `AllowedMethods` option.

```ruby
# bad
var.class == Date
var.class.equal?(Date)
var.class.eql?(Date)
var.class.name == 'Date'

# good
var.instance_of?(Date)
```

With `AllowedMethods: ['==', 'equal?', 'eql?']` (the default):

```ruby
# good
def ==(other)
  self.class == other.class && name == other.name
end
```

With `AllowedPatterns: []` (the default):

```ruby
# bad
def eq(other)
  self.class.eq(other.class) && name.eq(other.name)
end
```

# Safety

Autocorrection is unsafe because there is no guarantee that the constant named on the right-hand side actually exists when autocorrecting `var.class.name == 'Foo'` to `var.instance_of?(Foo)`.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `==`, `equal?`, `eql?` |  | Method names within which a class comparison is never flagged. |
| AllowedPatterns | `[]` |  | Method name regex patterns within which a class comparison is never flagged, checked the same way as `AllowedMethods`. |

## Blind spots

`AllowedPatterns` entries that fail to compile as a Rust regex are dropped (never match) rather than raising a configuration error.

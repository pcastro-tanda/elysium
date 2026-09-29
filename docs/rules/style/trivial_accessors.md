# Style/TrivialAccessors

Prefer attr_* methods to trivial readers/writers.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Looks for trivial reader/writer methods, that could have been created with
the attr_* family of functions automatically.

```ruby
# bad
def foo
  @foo
end

def bar=(val)
  @bar = val
end

def self.baz
  @baz
end

# good
attr_reader :foo
attr_writer :bar

class << self
  attr_reader :baz
end
```

`to_ary`, `to_a`, `to_c`, `to_enum`, `to_h`, `to_hash`, `to_i`, `to_int`,
`to_io`, `to_open`, `to_path`, `to_proc`, `to_r`, `to_regexp`, `to_str`,
`to_s`, and `to_sym` methods are allowed by default. These are customizable
with the `AllowedMethods` option.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| ExactNameMatch | true |  | Whether the ivar's name (minus `@`) must equal the method's name (minus `=`/`?`). |
| AllowPredicates | true |  | Whether a trivial `foo?` reader is allowed. |
| AllowDSLWriters | true |  | Whether a trivial writer whose name does not end in `=` (a DSL-style setter) is allowed. |
| IgnoreClassMethods | false |  | Whether `def self.foo`/`def self.foo=` are exempt entirely. |
| AllowedMethods | `to_ary`, `to_a`, `to_c`, `to_enum`, `to_h`, `to_hash`, `to_i`, `to_int`, `to_io`, `to_open`, `to_path`, `to_proc`, `to_r`, `to_regexp`, `to_str`, `to_s`, `to_sym` |  | Method names always allowed as trivial readers/writers (`initialize` is always allowed too, unconditionally). |

## Blind spots

None recorded.

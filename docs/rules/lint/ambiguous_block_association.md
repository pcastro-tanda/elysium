# Lint/AmbiguousBlockAssociation

Checks for ambiguous block association with method when param passed without parentheses.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | nursery |

This cop can customize allowed methods with `AllowedMethods`. By default,
there are no methods allowed.

```ruby
# bad
some_method a { |val| puts val }

# good
# With parentheses, there's no ambiguity.
some_method(a { |val| puts val })
# or (different meaning)
some_method(a) { |val| puts val }

# good
# Operator methods require no disambiguation
foo == bar { |b| b.baz }

# good
# Lambda arguments require no disambiguation
foo = ->(bar) { bar.baz }
```

With `AllowedMethods: [change]` (default: `[]`):

```ruby
# good
expect { do_something }.to change { object.attribute }
```

With `AllowedPatterns: ['change']` (default: `[]`):

```ruby
# good
expect { do_something }.to change { object.attribute }
expect { do_something }.to not_change { object.attribute }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `[]` |  | Method names always allowed to take an ambiguous block without parentheses. |
| AllowedPatterns | `[]` |  | Method name regex patterns always allowed to take an ambiguous block without parentheses. |

## Blind spots

The deprecated `IgnoredMethods`/`IgnoredPatterns`/`ExcludedMethods` config-key aliases (superseded
by `AllowedMethods`/`AllowedPatterns` since RuboCop 0.90/1.5) are not read; only the current keys
are. `AllowedPatterns` entries that fail to compile as a Rust regex are dropped (never match)
rather than raising a configuration error.

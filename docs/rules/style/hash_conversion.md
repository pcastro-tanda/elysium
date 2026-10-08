# Style/HashConversion

Avoid Hash[] in favor of ary.to_h or literal hashes.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks the usage of pre-2.1 `Hash[args]` method of converting enumerables and
sequences of values to hashes.

Correction code from splat argument (`Hash[*ary]`) is not simply determined. For example,
`Hash[*ary]` can be replaced with `ary.each_slice(2).to_h` but it will be complicated.
So, `AllowSplatArgument` option is true by default to allow splat argument for simple code.

@safety
This cop's autocorrection is unsafe because `ArgumentError` occurs
if the number of elements is odd:

```ruby
Hash[[[1, 2], [3]]] #=> {1=>2, 3=>nil}
[[1, 2], [5]].to_h  #=> wrong array length at 1 (expected 2, was 1) (ArgumentError)
```

```ruby
# bad
Hash[ary]

# good
ary.to_h

# bad
Hash[key1, value1, key2, value2]

# good
{key1 => value1, key2 => value2}
```

With `AllowSplatArgument: true` (default):

```ruby
# good
Hash[*ary]
```

With `AllowSplatArgument: false`:

```ruby
# bad
Hash[*ary]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowSplatArgument | true |  | Whether `Hash[*ary]` is allowed (its correction would require the more complex `ary.each_slice(2).to_h`, so it is unsafe to suggest automatically). |

## Blind spots

Only a braceless keyword-style argument (`Hash[a: b]`, Prism's `KeywordHashNode`) is treated as upstream's `hash_type?`; an explicit brace literal (`Hash[{a: 1}]`, Prism's `HashNode`) instead falls through to the generic `.to_h` branch, appending `.to_h` to its own verbatim source (`{a: 1}.to_h`) rather than splicing its contents into a replacement `{...}` literal.

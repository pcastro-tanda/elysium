# Layout/HashAlignment

Align the elements of a hash literal if they span more than one line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that the keys, separators, and values of a multi-line hash literal are
aligned according to the configured style.

```ruby
# EnforcedHashRocketStyle: key (default)
# bad
{
  :foo => bar,
   :ba => baz
}

# good
{
  :foo => bar,
  :ba => baz
}

# EnforcedHashRocketStyle: separator
# good
{
  :foo => bar,
   :ba => baz
}

# EnforcedHashRocketStyle: table
# good
{
  :foo => bar,
  :ba  => baz
}
```

The same three styles (`key`/`separator`/`table`) apply to colon-delimited
pairs via `EnforcedColonStyle`. Either option accepts a list of styles
instead of one; the style producing the fewest offenses wins.

`EnforcedLastArgumentHashStyle` controls whether a hash passed as the last
argument to a method call is inspected at all: `always_inspect` (default)
checks both implicit (`do_something(foo: 1,\n  bar: 2)`) and explicit
(`do_something({foo: 1,\n  bar: 2})`) hashes, `always_ignore` checks
neither, `ignore_implicit` skips only the braceless form, and
`ignore_explicit` skips only the braced form.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedHashRocketStyle | `key` | `key`, `separator`, `table` | Alignment of entries using a hash rocket (`=>`) as separator. |
| EnforcedColonStyle | `key` | `key`, `separator`, `table` | Alignment of entries using a colon (`:`) as separator. |
| EnforcedLastArgumentHashStyle | `always_inspect` | `always_inspect`, `always_ignore`, `ignore_implicit`, `ignore_explicit` | Whether a hash passed as the last call argument is inspected. |
| AllowMultipleStyles | true |  | Whether `EnforcedHashRocketStyle`/`EnforcedColonStyle` may list several styles. |

## Blind spots

- `autocorrect_incompatible_with_other_cops?`'s `same_line?` override
  (`node1.last_line == line(node2)`) is ported exactly, but only for a hash
  whose *direct* parent is a plain call (`CallNode`); RuboCop itself never
  applies it to `super`/`yield` either (`call_type?` excludes both).
- Keys/values containing non-ASCII text use plain character counts (matching
  RuboCop's own `Range#column`, which is not display-width aware here
  either), so no East-Asian-width blind spot beyond RuboCop's own.

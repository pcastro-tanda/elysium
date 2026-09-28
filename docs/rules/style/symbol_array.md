# Style/SymbolArray

Use %i or %I for arrays of symbols.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for array literals made up of symbols that are not using the `%i()`
syntax.

Alternatively, it checks for symbol arrays using the `%i()` syntax on
projects which do not want to use that syntax, perhaps because they support a
version of Ruby lower than 2.0.

The `MinSize` configuration option causes the cop to be ignored for arrays
smaller than the given value: a `MinSize` of `3` will not enforce a style on
an array of 2 or fewer elements.

```ruby
# EnforcedStyle: percent (default)

# good
%i[foo bar baz]

# bad
[:foo, :bar, :baz]

# bad (contains spaces)
%i[foo\ bar baz\ quux]

# bad (contains [] with spaces)
%i[foo \[ \]]

# bad (contains () with spaces)
%i(foo \( \))
```

```ruby
# EnforcedStyle: brackets

# good
[:foo, :bar, :baz]

# bad
%i[foo bar baz]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `percent` | `percent`, `brackets` | Whether symbol arrays should use `%i`/`%I` or `[...]` literals. |
| MinSize | 2 |  | Arrays with fewer elements than this are ignored. |

## Blind spots

`--auto-gen-config` bookkeeping (`array_style_detected`/`no_acceptable_style!`,
tracking the smallest percent array and largest bracket array seen across an
entire run to decide a project-wide style or disable the cop) is not ported:
it only affects `rubocop --auto-gen-config` output, never a single file's own
offenses, and this rule lints one file at a time.

`to_string_literal`'s encoding-aware branch assumes the process's
`Encoding.default_external` is UTF-8 (the overwhelmingly common case).
`PercentArray#invalid_percent_array_context?` is intentionally unported; see
the module doc comment.

# Style/HashLikeCase

Checks for places where `case-when` represents a simple 1:1 mapping and can be replaced with a hash lookup.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for places where `case-when` represents a simple 1:1
mapping and can be replaced with a hash lookup.

```ruby
# bad
case country
when 'europe'
  'http://eu.example.com'
when 'america'
  'http://us.example.com'
when 'australia'
  'http://au.example.com'
end

# good
SITES = {
  'europe'    => 'http://eu.example.com',
  'america'   => 'http://us.example.com',
  'australia' => 'http://au.example.com'
}
SITES[country]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| MinBranchesCount | 3 |  | Minimum number of branches a `case-when` needs to be flagged. |

## Blind spots

Bodies are recognized as literals by rubocop-ast's `recursive_basic_literal?`
(mirroring `Lint/DuplicateHashKey`'s existing port of the same method): plain
and interpolated strings/symbols/regexps, arrays, hashes, `and`/`or`,
ranges, parenthesized/interpolated single-statement wrappers, and the
comparison-operator/`!`/`*`/`<=>` sends rubocop-ast treats as recursively
literal. Rational and complex literals, and backtick/`%x` command strings,
are never recognized (a false-negative-only gap). A `MinBranchesCount` that
is not a positive integer falls back to the default of `3`, matching this
codebase's established fallback for the same shape of option in
`Style/GuardClause`'s `MinBodyLength`, rather than upstream's `raise`.

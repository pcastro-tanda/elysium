# Style/YodaCondition

Forbid or enforce yoda conditions.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Enforces or forbids Yoda conditions, i.e. comparison operations where the
order of expression is reversed, e.g. `5 == x`.

```ruby
# EnforcedStyle: forbid_for_all_comparison_operators (default)
# bad
99 == foo
"bar" != foo
42 >= foo
10 < bar
99 == CONST

# good
foo == 99
foo == "bar"
foo <= 42
bar > 10
CONST == 99
"#{interpolation}" == foo
/#{interpolation}/ == foo
```

```ruby
# EnforcedStyle: forbid_for_equality_operators_only
# bad
99 == foo
"bar" != foo

# good
99 >= foo
3 < a && a < 5
```

```ruby
# EnforcedStyle: require_for_all_comparison_operators
# bad
foo == 99
foo == "bar"
foo <= 42
bar > 10

# good
99 == foo
"bar" != foo
42 >= foo
10 < bar
```

```ruby
# EnforcedStyle: require_for_equality_operators_only
# bad
99 >= foo
3 < a && a < 5

# good
99 == foo
"bar" != foo
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `forbid_for_all_comparison_operators` | `forbid_for_all_comparison_operators`, `forbid_for_equality_operators_only`, `require_for_all_comparison_operators`, `require_for_equality_operators_only` | Which comparison operators are checked and in which direction. |

## Blind spots

This cop is unsafe: comparison operators can be defined differently on
different classes and are not guaranteed to have the same result if
reversed. `interpolation?(lhs)`'s `dstr_type?` half also exempts, upstream,
any multi-line plain string with no interpolation at all (whitequark
represents those as a `dstr` of per-line `str` children too); Prism parses a
multi-line literal with no interpolation as a single `StringNode`, so this
port -- unlike upstream -- can flag/require reordering a multi-line
non-interpolated string literal.

# Layout/FirstArgumentIndentation

Checks the indentation of the first argument in a method call.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Arguments after the first one are checked by `Layout/ArgumentAlignment`, not by this cop. For
indenting the first parameter of method _definitions_, check out `Layout/FirstParameterIndentation`.

This cop will respect `Layout/ArgumentAlignment` and will not work when `EnforcedStyle:
with_fixed_indentation` is specified for `Layout/ArgumentAlignment`.

```ruby
# bad
some_method(
first_param,
second_param)

# good (EnforcedStyle: special_for_inner_method_call_in_parentheses, the default)
some_method(
  first_param,
second_param)

# good (EnforcedStyle: consistent)
some_method(
  first_param,
second_param)

# good (EnforcedStyle: consistent_relative_to_receiver)
foo = some_method(
        first_param,
second_param)

# good (EnforcedStyle: special_for_inner_method_call)
some_method(
  first_param,
second_param)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `special_for_inner_method_call_in_parentheses` | `consistent`, `consistent_relative_to_receiver`, `special_for_inner_method_call`, `special_for_inner_method_call_in_parentheses` | The indentation style for the first argument of a multi-line method call. |
| IndentationWidth | `nil` |  | Overrides `Layout/IndentationWidth`'s configured width for this cop alone. |

## Blind spots

`enforce_first_argument_with_fixed_indentation?`/`enable_layout_first_method_argument_line_break?`
read `Layout/ArgumentAlignment`'s `EnforcedStyle` and `Layout/FirstMethodArgumentLineBreak`'s
`Enabled` through `peer(...)`, which only reflects an explicit key in that cop's own config block
in the loaded YAML, not a cop-wide default or `--only`/`--except` override; a file that relies on
either default to disable this cop is not detected (false negative only: this cop will still run
when RuboCop itself would have skipped it). `AlignmentCorrector`'s non-heredoc delimited-string
taboo ranges (plain
multi-line string/symbol literals) are not tracked, only heredoc bodies -- unlikely to matter for
a first-argument shift, since the argument being corrected is not itself one of those literals in
any fixture case.

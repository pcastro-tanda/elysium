# Layout/ArrayAlignment

Align the elements of an array literal if they span more than one line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that the elements of a multi-line array literal are aligned.

```ruby
# EnforcedStyle: with_first_element (default)

# good

array = [1, 2, 3,
         4, 5, 6]
array = ['run',
         'forrest',
         'run']

# bad

array = [1, 2, 3,
  4, 5, 6]
array = ['run',
     'forrest',
     'run']
```

```ruby
# EnforcedStyle: with_fixed_indentation

# good

array = [1, 2, 3,
  4, 5, 6]

# bad

array = [1, 2, 3,
         4, 5, 6]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `with_first_element` | `with_first_element`, `with_fixed_indentation` | Aligns following lines with the first element (`with_first_element`) or one indentation level past the line the array starts on (`with_fixed_indentation`). |
| IndentationWidth | `nil` |  | Overrides `Layout/IndentationWidth`'s configured width for `with_fixed_indentation`'s base column; falls back to it, else 2. |

## Blind spots

Autocorrection's taboo-range protection (RuboCop's `AlignmentCorrector`
`inside_string_ranges`) only covers heredoc bodies; the interior of an
ordinary multi-line quoted string or `%`-literal that itself begins a
physical line inside a misaligned element is not separately protected.
The block-comment guard is a per-line `=begin` text match rather than
resolving actual `EmbDoc` comment nodes, matching this crate's other
`Alignment`-based cops.

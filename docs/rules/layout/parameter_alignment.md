# Layout/ParameterAlignment

Align the parameters of a method definition if they span more than one line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that the parameters on a multi-line method call or definition are
aligned.

To set the alignment of the first argument, use the
`Layout/FirstParameterIndentation` cop.

```ruby
# EnforcedStyle: with_first_parameter (default)

# good

def foo(bar,
        baz)
  123
end

def foo(
  bar,
  baz
)
  123
end

# bad

def foo(bar,
     baz)
  123
end

# bad

def foo(
  bar,
     baz)
  123
end
```

```ruby
# EnforcedStyle: with_fixed_indentation

# good

def foo(bar,
  baz)
  123
end

def foo(
  bar,
  baz
)
  123
end

# bad

def foo(bar,
        baz)
  123
end

# bad

def foo(
  bar,
     baz)
  123
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `with_first_parameter` | `with_first_parameter`, `with_fixed_indentation` | Aligns following lines with the first parameter (`with_first_parameter`) or one indentation level past the line the method definition starts on (`with_fixed_indentation`). |
| IndentationWidth | `nil` |  | Overrides `Layout/IndentationWidth`'s configured width for `with_fixed_indentation`'s base column; falls back to it, else 2. |

## Blind spots

Autocorrection's taboo-range protection (RuboCop's `AlignmentCorrector`
`inside_string_ranges`) only covers heredoc bodies; the interior of an
ordinary multi-line quoted string or `%`-literal that itself begins a
physical line inside a misaligned parameter's default value is not
separately protected. The block-comment guard is a per-line `=begin` text
match rather than resolving actual `EmbDoc` comment nodes, matching this
crate's other `Alignment`-based cops.

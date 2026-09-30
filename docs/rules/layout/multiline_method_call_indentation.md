# Layout/MultilineMethodCallIndentation

Checks indentation of method calls with the dot operator that span more than one line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks the indentation of the method name part in method calls that span
more than one line.

```ruby
# EnforcedStyle: aligned (default)

# bad
while myvariable
.b
  # do something
end

# good
while myvariable
      .b
  # do something
end

# good
Thing.a
     .b
     .c
```

```ruby
# EnforcedStyle: indented

# good
while myvariable
  .b

  # do something
end
```

```ruby
# EnforcedStyle: indented_relative_to_receiver

# good
while myvariable
        .a
        .b

  # do something
end

# good
myvariable = Thing
               .a
               .b
               .c
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `aligned` | `aligned`, `indented`, `indented_relative_to_receiver` | Aligns the continuation's dot with the chain's first dot (`aligned`), indents it one width past the expression's own line (`indented`), or one width past the receiver (`indented_relative_to_receiver`). |
| IndentationWidth | `nil` |  | Overrides `Layout/IndentationWidth`'s configured width. Only accepted when `EnforcedStyle` is not `aligned`. |

## Blind spots

Autocorrection's taboo-range protection (RuboCop's `AlignmentCorrector`
`inside_string_ranges`) only covers heredoc bodies, and only for the
block-body part of a correction; the offense range itself is shifted as a
raw range, exactly as upstream passes it.

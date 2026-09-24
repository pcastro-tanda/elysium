# Lint/ElseLayout

Checks for odd code arrangement in an else block.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for odd `else` block layout -- like having an expression on the same
line as the `else` keyword, which is usually a mistake.

Its autocorrection tweaks layout to keep the syntax. So, this autocorrection
is compatible correction for bad case syntax, but if your code makes a
mistake with `elsif` and `else`, you will have to correct it manually.

```ruby
# bad

if something
  # ...
else do_this
  do_that
end

# good

# This code is compatible with the bad case. It will be autocorrected like
# this.
if something
  # ...
else
  do_this
  do_that
end

# This code is incompatible with the bad case.
# If `do_this` is a condition, `elsif` should be used instead of `else`.
if something
  # ...
elsif do_this
  do_that
end

# bad

# For single-line conditionals using `then` the layout is disallowed
# when the `else` body is multiline because it is treated as a lint offense.
if something then on_the_same_line_as_then
else first_line
  second_line
end

# good

# For single-line conditional using `then` the layout is allowed
# when `else` body is a single-line because it is treated as intentional.

if something then on_the_same_line_as_then
else single_line
end
```

## Options

This rule has no options.

## Blind spots

`unless`'s `else` is checked (whitequark's parser models `unless` as the
same `:if` node type RuboCop's `on_if` dispatches on); Prism gives it a
distinct `UnlessNode`, handled the same way here. An `elsif` chain is
walked one link at a time as the traversal visits each nested `IfNode`
rather than through RuboCop's explicit recursive `check`, which is
observationally identical (RuboCop's own offense-location dedup is why a
naive reading of its recursion does not double-report).

if a
  blah
else
  if b
  ^^ Convert `if` nested inside `else` to `elsif`.
    foo
  else # This is expected to be autocorrected by `Layout/IndentationWidth`.
    bar
  end
end

if a
  foo
else
  if b
  ^^ Convert `if` nested inside `else` to `elsif`.
    # TODO: comment.
  else
    bar
  end
end

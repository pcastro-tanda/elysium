if a
  blah
else
  if b
  ^^ Convert `if` nested inside `else` to `elsif`.
    foo
  elsif c # This is expected to be autocorrected by `Layout/IndentationWidth`.
      bar
  elsif d
    baz
  else
    qux
  end
end

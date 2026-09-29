if a
  blah
elsif b
  foo
else
  # important info
  bar if condition # blabla
      ^^ Convert `if` nested inside `else` to `elsif`.
end

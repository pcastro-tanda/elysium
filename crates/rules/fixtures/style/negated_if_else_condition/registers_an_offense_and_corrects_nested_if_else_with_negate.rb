if !x
^^^^^ Invert the negated condition and swap the if-else branches.
  do_something
else
  if !y
  ^^^^^ Invert the negated condition and swap the if-else branches.
    do_something_else_1
  else
    do_something_else_2
  end
end

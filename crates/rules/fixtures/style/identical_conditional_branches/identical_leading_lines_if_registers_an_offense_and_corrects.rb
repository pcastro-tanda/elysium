x = if foo
  bar
  ^^^ Move `bar` out of the conditional.
  do_x(1)
else
  bar
  ^^^ Move `bar` out of the conditional.
  do_x(2)
end

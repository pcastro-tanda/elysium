x = case something
in :a
  bar
  ^^^ Move `bar` out of the conditional.
  do_x(1)
in :b
  bar
  ^^^ Move `bar` out of the conditional.
  do_x(2)
else
  bar
  ^^^ Move `bar` out of the conditional.
  do_x(3)
end

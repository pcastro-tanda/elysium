case something
in :a
  do_x
  ^^^^ Move `do_x` out of the conditional.
  x1
in :b
  do_x
  ^^^^ Move `do_x` out of the conditional.
  x2
else
  do_x
  ^^^^ Move `do_x` out of the conditional.
  x3
end

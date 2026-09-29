case something
in :a
  do_x
  ^^^^ Move `do_x` out of the conditional.
in :b
  do_x
  ^^^^ Move `do_x` out of the conditional.
else
  do_x
  ^^^^ Move `do_x` out of the conditional.
end

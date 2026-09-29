if something
  method_call_here(1, 2, 3)
  do_x
  ^^^^ Move `do_x` out of the conditional.
else
  1 + 2 + 3
  do_x
  ^^^^ Move `do_x` out of the conditional.
end

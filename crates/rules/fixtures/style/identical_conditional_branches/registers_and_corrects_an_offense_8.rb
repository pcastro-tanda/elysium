case something
when :a
  do_x
  ^^^^ Move `do_x` out of the conditional.
  x1
when :b
  do_x
  ^^^^ Move `do_x` out of the conditional.
  x2
else
  do_x
  ^^^^ Move `do_x` out of the conditional.
  x3
end

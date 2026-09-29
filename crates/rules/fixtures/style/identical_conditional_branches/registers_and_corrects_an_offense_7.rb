case something
when :a
  x1
  do_x
  ^^^^ Move `do_x` out of the conditional.
when :b
  x2
  do_x
  ^^^^ Move `do_x` out of the conditional.
else
  x3
  do_x
  ^^^^ Move `do_x` out of the conditional.
end

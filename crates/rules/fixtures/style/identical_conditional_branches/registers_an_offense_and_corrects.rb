case something
when :a
  do_x
  ^^^^ Move `do_x` out of the conditional.
when :b
  do_x
  ^^^^ Move `do_x` out of the conditional.
else
  do_x
  ^^^^ Move `do_x` out of the conditional.
end

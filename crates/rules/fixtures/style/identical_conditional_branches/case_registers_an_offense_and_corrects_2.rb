x = case something
when :a
  bar
  ^^^ Move `bar` out of the conditional.
  do_x(1)
when :b
  bar
  ^^^ Move `bar` out of the conditional.
  do_x(2)
else
  bar
  ^^^ Move `bar` out of the conditional.
  do_x(3)
end

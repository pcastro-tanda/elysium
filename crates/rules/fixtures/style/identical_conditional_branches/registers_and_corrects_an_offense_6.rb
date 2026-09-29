if x.condition
  foo
  x = do_something
  ^^^^^^^^^^^^^^^^ Move `x = do_something` out of the conditional.
else
  bar
  x = do_something
  ^^^^^^^^^^^^^^^^ Move `x = do_something` out of the conditional.
end

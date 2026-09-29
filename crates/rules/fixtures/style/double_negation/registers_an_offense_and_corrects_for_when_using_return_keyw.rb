def foo?
  return !!bar.do_something if condition
         ^ Avoid the use of double negation (`!!`).
  baz
  !!bar
  ^ Avoid the use of double negation (`!!`).
end

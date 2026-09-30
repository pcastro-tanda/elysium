format(
  case condition
  when foo
    bar
  else
    baz
end, qux
^^^ `end` at 7, 0 is not aligned with `case` at 2, 2.
)

unless foo
  if foobar
  ^^^^^^^^^ Use the return of the conditional for variable assignment and comparison.
    baz = 1
  elsif qux
    baz = 2
  else
    baz = 3
  end
else
  baz = 4
end

case :a
when false
  foo
when nil, false, 1
  bar
when false, 1
  baz
end

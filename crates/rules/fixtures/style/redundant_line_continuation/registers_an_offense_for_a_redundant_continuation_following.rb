x do
  foo bar \
    baz
end

y do
  foo(bar, \
           ^ Redundant line continuation.
    baz)
end

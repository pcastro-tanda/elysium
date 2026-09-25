def some_method
  foo, (bar, (baz, qux)) = do_something
                   ^^^ Useless assignment to variable - `qux`. Use `_` or `_qux` as a variable name to indicate that it won't be used.
  puts foo, bar, baz
end

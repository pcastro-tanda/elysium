def some_method
  (foo, bar), *baz = do_something
               ^^^ Useless assignment to variable - `baz`. Use `_` or `_baz` as a variable name to indicate that it won't be used.
  puts foo, bar
end

def some_method
  foo, (*bar, baz) = do_something
         ^^^ Useless assignment to variable - `bar`. Use `_` or `_bar` as a variable name to indicate that it won't be used.
  puts foo, baz
end

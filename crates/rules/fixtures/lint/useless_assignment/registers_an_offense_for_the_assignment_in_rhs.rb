def some_method
  foo = 1
  foo += foo = 2
         ^^^ Useless assignment to variable - `foo`.
  foo
end

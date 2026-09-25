def some_method
  foo = 1
  ^^^ Useless assignment to variable - `foo`.
  bar = 2
  foo = 3
  ^^^ Useless assignment to variable - `foo`.
  puts bar
end

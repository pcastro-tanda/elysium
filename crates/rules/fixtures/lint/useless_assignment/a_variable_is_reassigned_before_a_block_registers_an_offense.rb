def some_method
  foo = 1
  ^^^ Useless assignment to variable - `foo`.
  foo = 2
  bar {
    foo = 3
  }
end

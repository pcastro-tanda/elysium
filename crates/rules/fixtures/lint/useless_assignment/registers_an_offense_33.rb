def some_method(foo, bar)
  foo = 1
  ^^^ Useless assignment to variable - `foo`.
  super(bar)
end

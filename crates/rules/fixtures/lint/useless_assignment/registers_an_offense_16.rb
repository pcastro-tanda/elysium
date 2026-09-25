def some_method(flag)
  foo = 1 unless foo
  ^^^ Useless assignment to variable - `foo`.
end

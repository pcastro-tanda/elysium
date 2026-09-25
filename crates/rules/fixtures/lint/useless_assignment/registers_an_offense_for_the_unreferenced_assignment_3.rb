def some_method(flag)
  foo = 1
  ^^^ Useless assignment to variable - `foo`.

  if flag
    foo = 2
    puts foo
  end
end

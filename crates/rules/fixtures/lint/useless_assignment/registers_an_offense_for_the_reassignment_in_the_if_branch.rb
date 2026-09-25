def some_method(flag)
  foo = 1

  if flag
    puts foo
    foo = 2
    ^^^ Useless assignment to variable - `foo`.
  else
    puts foo
  end
end

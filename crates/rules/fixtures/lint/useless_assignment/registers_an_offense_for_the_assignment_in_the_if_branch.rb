def some_method(flag)
  if flag
    foo = 2
    ^^^ Useless assignment to variable - `foo`.
  else
    foo = 3
    puts foo
  end
end

def some_method(flag)
  if flag
    foo = 1
    ^^^ Useless assignment to variable - `foo`.
  end
end

def some_method(flag)
  if flag
    foo = 1
    ^^^ Useless assignment to variable - `foo`.
    foo = 2
  end

  foo
end

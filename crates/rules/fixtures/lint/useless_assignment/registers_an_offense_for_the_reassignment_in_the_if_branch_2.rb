def some_method(flag_a, flag_b)
  foo = 1

  if flag_a
    puts foo
    foo = 2
    ^^^ Useless assignment to variable - `foo`.
  elsif flag_b
    puts foo
  end
end

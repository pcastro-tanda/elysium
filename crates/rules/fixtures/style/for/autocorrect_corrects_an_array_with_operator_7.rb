def func
  a = nil
  b = [1, 2, 3]

  for n in a || b
  ^^^^^^^^^^^^^^^ Prefer `each` over `for`.
    puts n
  end
end

def func
  a = [1, 2, 3, 4]
  b = [3]

  for n in a - b
  ^^^^^^^^^^^^^^ Prefer `each` over `for`.
    puts n
  end
end

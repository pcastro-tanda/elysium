def func
  a = [1, 2, 3, 4]
  b = [4, 5]

  for n in a & b
  ^^^^^^^^^^^^^^ Prefer `each` over `for`.
    puts n
  end
end

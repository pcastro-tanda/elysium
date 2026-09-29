def func
  a = [1, 2]
  b = [3, 4]
  c = [5]

  for n in a + b + c
  ^^^^^^^^^^^^^^^^^^ Prefer `each` over `for`.
    puts n
  end
end

def func
  for n in [1, 2, 3, 4] * 3
  ^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `each` over `for`.
    puts n
  end
end

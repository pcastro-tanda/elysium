def func
  for n in [1, 2, nil].compact
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `each` over `for`.
    puts n
  end
end

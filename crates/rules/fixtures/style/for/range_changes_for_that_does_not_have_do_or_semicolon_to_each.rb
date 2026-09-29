def func
  for n in (1...value)
  ^^^^^^^^^^^^^^^^^^^^ Prefer `each` over `for`.
    puts n
  end
end

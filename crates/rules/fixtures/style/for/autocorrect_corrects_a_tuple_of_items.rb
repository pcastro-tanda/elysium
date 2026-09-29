def func
  for (a, b) in {a: 1, b: 2, c: 3} do
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `each` over `for`.
    puts a, b
  end
end

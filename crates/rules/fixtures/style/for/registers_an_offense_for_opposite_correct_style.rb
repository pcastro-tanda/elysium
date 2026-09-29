def func
  for n in [1, 2, 3] do
  ^^^^^^^^^^^^^^^^^^^^^ Prefer `each` over `for`.
    puts n
  end
  [1, 2, 3].each do |n|
    puts n
  end
end

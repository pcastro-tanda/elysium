def func
  for n in [1, 2, 3] do
    puts n
  end
  [1, 2, 3].each do |n|
  ^^^^^^^^^^^^^^^^^^^^^ Prefer `for` over `each`.
    puts n
  end
end

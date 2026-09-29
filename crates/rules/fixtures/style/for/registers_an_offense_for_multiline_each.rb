def func
  [1, 2, 3].each do |n|
  ^^^^^^^^^^^^^^^^^^^^^ Prefer `for` over `each`.
    puts n
  end
end

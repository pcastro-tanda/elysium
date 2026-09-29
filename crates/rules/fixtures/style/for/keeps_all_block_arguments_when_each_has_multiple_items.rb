def func
  [[1, 2]].each do |a, b|
  ^^^^^^^^^^^^^^^^^^^^^^^ Prefer `for` over `each`.
    puts a + b
  end
end

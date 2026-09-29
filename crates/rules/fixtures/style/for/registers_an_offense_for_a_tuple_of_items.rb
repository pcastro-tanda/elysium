def func
  {a: 1, b: 2, c: 3}.each do |(a, b)|
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `for` over `each`.
    puts a, b
  end
end

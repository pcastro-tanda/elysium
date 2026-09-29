def m
  items.something { |i| yield i }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Consider using explicit block argument in the surrounding method's signature over `yield`.

  if condition
    yield 2
  elsif other_condition
    3.times { yield }
    ^^^^^^^^^^^^^^^^^ Consider using explicit block argument in the surrounding method's signature over `yield`.
  else
    other_items.something { |i, j| yield i, j }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Consider using explicit block argument in the surrounding method's signature over `yield`.
  end
end

def func
  for n in (1...value) do
  ^^^^^^^^^^^^^^^^^^^^^^^ Prefer `each` over `for`.
    puts n
  end
end

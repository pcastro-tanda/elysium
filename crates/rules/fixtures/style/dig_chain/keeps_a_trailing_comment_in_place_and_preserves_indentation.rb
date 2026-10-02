def m
  foo.dig(:a).dig(:b) # c
      ^^^^^^^^^^^^^^^ Use `dig(:a, :b)` instead of chaining.
end

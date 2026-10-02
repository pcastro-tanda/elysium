if a + b < low
^^^^^^^^^^^^^^ Use `(a + b).clamp(low, high)` instead of `if/elsif/else`.
  low
elsif high < a + b
  high
else
  a + b
end

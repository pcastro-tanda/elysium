if x < low
^^^^^^^^^^ Use `x.clamp(low, high)` instead of `if/elsif/else`.
  low
elsif high < x
  high
else
  x
end

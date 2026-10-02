if x < low
^^^^^^^^^^ Use `x.clamp(low, high)` instead of `if/elsif/else`.
  low
elsif x > high
  high
else
  x
end

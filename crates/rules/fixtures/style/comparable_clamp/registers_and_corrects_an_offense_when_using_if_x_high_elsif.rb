if x > high
^^^^^^^^^^^ Use `x.clamp(low, high)` instead of `if/elsif/else`.
  high
elsif x < low
  low
else
  x
end

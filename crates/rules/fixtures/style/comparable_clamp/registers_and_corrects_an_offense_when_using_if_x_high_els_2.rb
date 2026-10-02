if x > high
^^^^^^^^^^^ Use `x.clamp(low, high)` instead of `if/elsif/else`.
  high
elsif low > x
  low
else
  x
end

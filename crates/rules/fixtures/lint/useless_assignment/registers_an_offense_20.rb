def some_method
  a, b = func(c = 3)
              ^ Useless assignment to variable - `c`.
  [a, b]
end

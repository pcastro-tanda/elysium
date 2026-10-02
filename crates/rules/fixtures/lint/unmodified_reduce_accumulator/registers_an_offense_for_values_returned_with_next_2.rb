(1..4).inject(0) do |acc, el|
  next el if el.even?
       ^^ Ensure the accumulator `acc` will be modified by `inject`.
  acc += 1
end

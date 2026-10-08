(1..4).reduce(0) do |acc, el|
  next el if el.even?
       ^^ Ensure the accumulator `acc` will be modified by `reduce`.
  acc += 1
end

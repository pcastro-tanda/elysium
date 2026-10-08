values.reduce({}) do |acc, el|
  acc[el] = true
  el
  ^^ Ensure the accumulator `acc` will be modified by `reduce`.
end

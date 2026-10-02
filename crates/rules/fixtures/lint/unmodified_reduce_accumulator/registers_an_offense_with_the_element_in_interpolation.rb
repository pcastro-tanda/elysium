(1..4).reduce(0) do |acc, el|
  "#{el}"
  ^^^^^^^ Ensure the accumulator `acc` will be modified by `reduce`.
end

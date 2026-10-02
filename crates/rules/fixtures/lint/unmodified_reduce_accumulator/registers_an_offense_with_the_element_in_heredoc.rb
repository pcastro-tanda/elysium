(1..4).reduce(0) do |acc, el|
  <<~RESULT
  ^^^^^^^^^ Ensure the accumulator `acc` will be modified by `reduce`.
    #{el}
  RESULT
end

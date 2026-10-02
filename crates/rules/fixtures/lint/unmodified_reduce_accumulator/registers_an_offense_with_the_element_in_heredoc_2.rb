(1..4).inject(0) do |acc, el|
  <<~RESULT
  ^^^^^^^^^ Ensure the accumulator `acc` will be modified by `inject`.
    #{el}
  RESULT
end

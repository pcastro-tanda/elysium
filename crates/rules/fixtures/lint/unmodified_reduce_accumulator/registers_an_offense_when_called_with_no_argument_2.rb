(1..4).inject do |acc, el|
  el
  ^^ Ensure the accumulator `acc` will be modified by `inject`.
end

(1..4).reduce do |acc, el|
  el
  ^^ Ensure the accumulator `acc` will be modified by `reduce`.
end

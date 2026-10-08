(1..4).each_with_index.reduce([]) do |acc, (el, index)|
  acc[el] = method(index)
  el
  ^^ Ensure the accumulator `acc` will be modified by `reduce`.
end

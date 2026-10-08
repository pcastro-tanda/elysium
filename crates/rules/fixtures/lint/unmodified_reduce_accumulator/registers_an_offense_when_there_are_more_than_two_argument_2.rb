(1..4).each_with_index.inject([]) do |acc, (el, index)|
  acc[el] = method(index)
  el
  ^^ Ensure the accumulator `acc` will be modified by `inject`.
end

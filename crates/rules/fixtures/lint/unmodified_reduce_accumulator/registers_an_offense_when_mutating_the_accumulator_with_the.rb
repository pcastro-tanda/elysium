values.reduce do |acc, el|
  acc = el
  acc += el
  acc << el
  acc &&= el
  acc.method!(el)
  el
  ^^ Ensure the accumulator `acc` will be modified by `reduce`.
end

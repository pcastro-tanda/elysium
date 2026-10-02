%w(a b c).reduce({}) do |acc, letter|
  acc[foo]
  ^^^^^^^^ Do not return an element of the accumulator in `reduce`.
end

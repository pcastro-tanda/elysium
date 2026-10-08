%w(a b c).inject({}) do |acc, letter|
  acc[foo]
  ^^^^^^^^ Do not return an element of the accumulator in `inject`.
end

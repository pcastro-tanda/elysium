%w(a b c).inject({}) do |acc, letter|
  acc[foo] = bar
  ^^^^^^^^^^^^^^ Do not return an element of the accumulator in `inject`.
end

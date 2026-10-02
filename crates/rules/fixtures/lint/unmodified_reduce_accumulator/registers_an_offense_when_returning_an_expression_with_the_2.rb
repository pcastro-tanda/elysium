(1..4).inject(0) do |acc, el|
  el + 2
  ^^^^^^ Ensure the accumulator `acc` will be modified by `inject`.
end

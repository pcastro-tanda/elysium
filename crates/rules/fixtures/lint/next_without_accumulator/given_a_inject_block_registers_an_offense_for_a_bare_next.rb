(1..4).inject(0) do |acc, i|
  next if i.odd?
  ^^^^ Use `next` with an accumulator argument in a `reduce`.
  acc + i
end

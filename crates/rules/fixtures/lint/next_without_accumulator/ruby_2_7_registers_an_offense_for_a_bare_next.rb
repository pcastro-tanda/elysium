(1..4).reduce(0) do
  next if _2.odd?
  ^^^^ Use `next` with an accumulator argument in a `reduce`.
  _1 + i
end

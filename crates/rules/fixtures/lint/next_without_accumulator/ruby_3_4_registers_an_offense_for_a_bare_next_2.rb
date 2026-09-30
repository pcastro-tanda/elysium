(1..4).inject(0) do
  next if it.odd?
  ^^^^ Use `next` with an accumulator argument in a `reduce`.
  it + 1
end

(1..4).reduce(0) do |acc, i|
  next acc if i.odd?
  acc + i
end

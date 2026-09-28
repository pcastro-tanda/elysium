(1..4).inject(0) do |acc, i|
  next acc if i.odd?
  acc + i
end

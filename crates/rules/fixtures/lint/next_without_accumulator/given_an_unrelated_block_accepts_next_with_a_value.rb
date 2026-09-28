(1..4).foo(0) do |acc, i|
  next acc if i.odd?
  acc + i
end

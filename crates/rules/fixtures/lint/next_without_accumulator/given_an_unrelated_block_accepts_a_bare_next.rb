(1..4).foo(0) do |acc, i|
  next if i.odd?
  acc + i
end

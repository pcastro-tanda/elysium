(1..4).reduce(0) do |acc, el|
  el &&= acc
end

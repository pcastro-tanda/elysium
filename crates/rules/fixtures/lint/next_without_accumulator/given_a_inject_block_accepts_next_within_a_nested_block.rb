[(1..3), (4..6)].inject(0) do |acc, elems|
  elems.each_with_index do |elem, i|
    next if i == 1
    acc << elem
  end
  acc
end

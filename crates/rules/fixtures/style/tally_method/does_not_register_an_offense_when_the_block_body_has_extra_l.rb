array.each_with_object(Hash.new(0)) do |item, counts|
  counts[item] += 1
  puts item
end

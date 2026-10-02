array.each_with_object({}) do |elem, hash|
  hash[elem.id] = elem.name
  puts elem
end

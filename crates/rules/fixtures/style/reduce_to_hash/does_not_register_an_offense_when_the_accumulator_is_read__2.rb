array.each_with_object({}) do |elem, hash|
  hash[hash.size] = elem
end

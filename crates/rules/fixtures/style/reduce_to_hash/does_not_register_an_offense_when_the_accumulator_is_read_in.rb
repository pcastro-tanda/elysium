array.each_with_object({}) do |elem, hash|
  hash[elem.id] = hash[elem.id].to_i + 1
end

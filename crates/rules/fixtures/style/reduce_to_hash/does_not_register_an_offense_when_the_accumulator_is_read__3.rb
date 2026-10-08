array.inject({}) do |hash, elem|
  hash[elem.id] = hash[elem.id].to_i + 1
  hash
end

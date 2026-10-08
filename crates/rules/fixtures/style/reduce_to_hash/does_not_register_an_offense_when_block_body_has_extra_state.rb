array.inject({}) do |hash, elem|
  puts elem
  hash[elem] = true
  hash
end

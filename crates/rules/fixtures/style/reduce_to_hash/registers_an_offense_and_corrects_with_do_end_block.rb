array.each_with_object({}) do |elem, hash|
      ^^^^^^^^^^^^^^^^ Use `to_h { ... }` instead of `each_with_object`.
  hash[elem.id] = elem.name
end

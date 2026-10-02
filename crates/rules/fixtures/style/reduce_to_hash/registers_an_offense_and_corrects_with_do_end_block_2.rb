array.inject({}) do |hash, elem|
      ^^^^^^ Use `to_h { ... }` instead of `inject`.
  hash[elem.id] = elem.name
  hash
end

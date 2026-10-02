foo.map do |item|
    ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `select`.
  next nil if item.bar?

  item
end.compact

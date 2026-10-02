foo.map do |item|
    ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `select`.
  next item if item.bar?
end.compact

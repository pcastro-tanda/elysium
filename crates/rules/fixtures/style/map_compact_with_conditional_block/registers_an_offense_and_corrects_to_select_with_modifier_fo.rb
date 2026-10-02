foo.map do |item|
    ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `select`.
  item if item.bar?
end.compact

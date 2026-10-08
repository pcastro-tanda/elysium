foo.map do |item|
    ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `select`.
  item.bar? ? item : next
end.compact

foo.map do |item|
    ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `reject`.
  item.bar? ? next : item
end.compact

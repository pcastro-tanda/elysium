foo.map do |item|
    ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `reject`.
  item unless item.bar?
end.compact

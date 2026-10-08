foo.map do |item|
    ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `reject`.
  unless item.bar?
    item
  end
end.compact

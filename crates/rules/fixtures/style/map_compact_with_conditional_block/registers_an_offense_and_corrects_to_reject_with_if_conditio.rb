foo.map do |item|
    ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `reject`.
  if item.bar?
    next
  else
    item
  end
end.compact

foo.map do |item|
    ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `reject`.
  if item.bar?
    nil
  else
    next item
  end
end.compact

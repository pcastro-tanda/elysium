foo.map do |item|
    ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `select`.
  if item.bar?
    next item
  else
    nil
  end
end.compact

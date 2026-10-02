foo.map do |item|
    ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `select`.
  if item.bar? &&
    bar.baz
    item
  else
    next
  end
end.compact

foo.filter_map do |item|
    ^^^^^^^^^^^^^^^^^^^^ Replace `filter_map { ... }.compact` with `select`.
  if item.bar?
    item
  else
    next
  end
end.compact

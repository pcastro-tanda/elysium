foo.filter_map do |item|
    ^^^^^^^^^^^^^^^^^^^^ Replace `filter_map { ... }` with `select`.
  if item.bar?
    item
  else
    next
  end
end

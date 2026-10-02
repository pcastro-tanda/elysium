foo&.map do |item|
     ^^^^^^^^^^^^^ Replace `map { ... }.compact` with `select`.
  if item.bar?
    item
  else
    next
  end
end&.compact

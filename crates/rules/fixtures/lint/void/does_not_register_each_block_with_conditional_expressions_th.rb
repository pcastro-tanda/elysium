enumerator_as_filter.each do |item|
  puts item

  # The `filter` method is used to filter for matches with `42`.
  # In this case, it's not void.
  item == 42
end
